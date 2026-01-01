#![forbid(unsafe_code)]

use qw_common::Vec3;

pub const PROG_VERSION: i32 = 6;
const DEF_SAVEGLOBAL: i16 = 1 << 15;
const PARAM_SLOT_SIZE: usize = 3;
const OFS_RETURN: usize = 0;
const OFS_PARM0: usize = 4;

#[derive(Debug)]
pub enum ProgsError {
    BufferTooSmall,
    UnsupportedVersion(i32),
    InvalidLump(&'static str),
    InvalidStringOffset(i32),
    InvalidUtf8,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Statement {
    pub op: u16,
    pub a: i16,
    pub b: i16,
    pub c: i16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QcType {
    Void,
    String,
    Float,
    Vector,
    Entity,
    Field,
    Function,
    Pointer,
    Integer,
    Unknown(i16),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Definition {
    pub ty: QcType,
    pub offset: i16,
    pub name: String,
    pub save_global: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub first_statement: i32,
    pub parm_start: i32,
    pub locals: i32,
    pub profile: i32,
    pub name: String,
    pub file: String,
    pub num_params: i32,
    pub param_sizes: [u8; 8],
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProgsDat {
    pub version: i32,
    pub crc: i32,
    pub statements: Vec<Statement>,
    pub global_defs: Vec<Definition>,
    pub field_defs: Vec<Definition>,
    pub functions: Vec<Function>,
    pub strings: Vec<u8>,
    pub globals: Vec<u32>,
    pub entity_fields: i32,
}

impl ProgsDat {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ProgsError> {
        if bytes.len() < 60 {
            return Err(ProgsError::BufferTooSmall);
        }

        let version = read_i32(bytes, 0)?;
        if version != PROG_VERSION {
            return Err(ProgsError::UnsupportedVersion(version));
        }
        let crc = read_i32(bytes, 4)?;

        let statements_lump = read_lump(bytes, 8)?;
        let globaldefs_lump = read_lump(bytes, 16)?;
        let fielddefs_lump = read_lump(bytes, 24)?;
        let functions_lump = read_lump(bytes, 32)?;
        let strings_lump = read_lump(bytes, 40)?;
        let globals_lump = read_lump(bytes, 48)?;
        let entity_fields = read_i32(bytes, 56)?;

        let strings_size = strings_lump
            .count
            .try_into()
            .ok()
            .ok_or(ProgsError::InvalidLump("strings"))?;
        let strings = read_bytes(bytes, strings_lump, "strings", strings_size)?;
        let statements = read_statements(bytes, statements_lump)?;
        let global_defs = read_defs(bytes, globaldefs_lump, &strings)?;
        let field_defs = read_defs(bytes, fielddefs_lump, &strings)?;
        let functions = read_functions(bytes, functions_lump, &strings)?;
        let globals = read_globals(bytes, globals_lump)?;

        Ok(Self {
            version,
            crc,
            statements,
            global_defs,
            field_defs,
            functions,
            strings,
            globals,
            entity_fields,
        })
    }

    pub fn function_index(&self, name: &str) -> Option<usize> {
        self.functions
            .iter()
            .position(|func| func.name.eq_ignore_ascii_case(name))
    }

    pub fn global_def(&self, name: &str) -> Option<&Definition> {
        self.global_defs
            .iter()
            .find(|def| def.name.eq_ignore_ascii_case(name))
    }

    pub fn field_def(&self, name: &str) -> Option<&Definition> {
        self.field_defs
            .iter()
            .find(|def| def.name.eq_ignore_ascii_case(name))
    }

    pub fn string_at(&self, offset: i32) -> Result<String, ProgsError> {
        read_cstring(&self.strings, offset)
    }
}

#[derive(Debug)]
pub enum VmError {
    BadGlobal(i16),
    BadStatement(i32),
    BadFunction(i32),
    BadEdict(i32),
    BadField(i32),
    UnsupportedOpcode(u16),
    BuiltinNotRegistered(i32),
    StepLimit,
}

type VmResult<T> = Result<T, VmError>;
pub type BuiltinFn = fn(&mut Vm) -> VmResult<()>;

#[derive(Debug, Clone)]
struct CallFrame {
    function_index: usize,
    statement_index: i32,
    local_base: usize,
    locals: usize,
    return_statement: Option<i32>,
}

#[derive(Debug, Clone)]
struct Edict {
    fields: Vec<u32>,
}

impl Edict {
    fn new(field_count: usize) -> Self {
        Self {
            fields: vec![0; field_count],
        }
    }
}

pub struct Vm {
    progs: ProgsDat,
    globals: Vec<u32>,
    local_stack: Vec<u32>,
    call_stack: Vec<CallFrame>,
    edicts: Vec<Edict>,
    builtins: Vec<Option<BuiltinFn>>,
}

impl Vm {
    pub fn new(progs: ProgsDat) -> Self {
        let globals = progs.globals.clone();
        let field_count = progs.entity_fields.max(0) as usize;
        let edicts = vec![Edict::new(field_count)];
        Self {
            progs,
            globals,
            local_stack: Vec::new(),
            call_stack: Vec::new(),
            edicts,
            builtins: Vec::new(),
        }
    }

    pub fn progs(&self) -> &ProgsDat {
        &self.progs
    }

    pub fn register_builtin(&mut self, index: usize, func: BuiltinFn) {
        if self.builtins.len() <= index {
            self.builtins.resize_with(index + 1, || None);
        }
        self.builtins[index] = Some(func);
    }

    pub fn global_def(&self, name: &str) -> Option<&Definition> {
        self.progs.global_def(name)
    }

    pub fn field_def(&self, name: &str) -> Option<&Definition> {
        self.progs.field_def(name)
    }

    pub fn edict_field_count(&self) -> usize {
        self.progs.entity_fields.max(0) as usize
    }

    pub fn edict_count(&self) -> usize {
        self.edicts.len()
    }

    pub fn alloc_edict(&mut self) -> usize {
        let index = self.edicts.len();
        let field_count = self.edict_field_count();
        self.edicts.push(Edict::new(field_count));
        index
    }

    pub fn call_by_name(&mut self, name: &str, max_steps: usize) -> VmResult<()> {
        let index = self
            .progs
            .function_index(name)
            .ok_or(VmError::BadFunction(-1))?;
        self.call_function(index, max_steps)
    }

    pub fn call_function(&mut self, index: usize, max_steps: usize) -> VmResult<()> {
        self.enter_function(index, None)?;
        self.execute(max_steps)
    }

    pub fn read_global_raw(&self, ofs: i16) -> VmResult<u32> {
        self.read_raw(ofs)
    }

    pub fn write_global_raw(&mut self, ofs: i16, value: u32) -> VmResult<()> {
        self.write_raw(ofs, value)
    }

    pub fn read_global_f32(&self, ofs: i16) -> VmResult<f32> {
        self.read_f32(ofs)
    }

    pub fn write_global_f32(&mut self, ofs: i16, value: f32) -> VmResult<()> {
        self.write_f32(ofs, value)
    }

    pub fn read_global_vec(&self, ofs: i16) -> VmResult<Vec3> {
        self.read_vec(ofs)
    }

    pub fn write_global_vec(&mut self, ofs: i16, value: Vec3) -> VmResult<()> {
        self.write_vec(ofs, value)
    }

    pub fn read_edict_field_raw(
        &self,
        entity: usize,
        field: usize,
        count: usize,
    ) -> VmResult<Vec<u32>> {
        self.edict_field(entity, field, count)
    }

    pub fn write_edict_field_raw(
        &mut self,
        entity: usize,
        field: usize,
        values: &[u32],
    ) -> VmResult<()> {
        let edict = self
            .edicts
            .get_mut(entity)
            .ok_or(VmError::BadEdict(entity as i32))?;
        let end = field + values.len();
        if end > edict.fields.len() {
            return Err(VmError::BadField(field as i32));
        }
        edict.fields[field..end].copy_from_slice(values);
        Ok(())
    }

    pub fn read_edict_field_f32(&self, entity: usize, field: usize) -> VmResult<f32> {
        let value = self
            .read_edict_field_raw(entity, field, 1)?
            .first()
            .copied()
            .unwrap_or(0);
        Ok(f32::from_bits(value))
    }

    pub fn write_edict_field_f32(
        &mut self,
        entity: usize,
        field: usize,
        value: f32,
    ) -> VmResult<()> {
        let values = [value.to_bits()];
        self.write_edict_field_raw(entity, field, &values)
    }

    pub fn read_edict_field_vec(&self, entity: usize, field: usize) -> VmResult<Vec3> {
        let values = self.read_edict_field_raw(entity, field, 3)?;
        let x = values.first().copied().unwrap_or(0);
        let y = values.get(1).copied().unwrap_or(0);
        let z = values.get(2).copied().unwrap_or(0);
        Ok(Vec3::new(
            f32::from_bits(x),
            f32::from_bits(y),
            f32::from_bits(z),
        ))
    }

    pub fn write_edict_field_vec(
        &mut self,
        entity: usize,
        field: usize,
        value: Vec3,
    ) -> VmResult<()> {
        let values = [value.x.to_bits(), value.y.to_bits(), value.z.to_bits()];
        self.write_edict_field_raw(entity, field, &values)
    }

    fn execute(&mut self, max_steps: usize) -> VmResult<()> {
        let mut steps = 0usize;
        while !self.call_stack.is_empty() {
            if steps >= max_steps {
                return Err(VmError::StepLimit);
            }
            steps += 1;

            let frame_index = self.call_stack.len() - 1;
            let statement_index = self.call_stack[frame_index].statement_index;
            let statement = self
                .progs
                .statements
                .get(statement_index as usize)
                .ok_or(VmError::BadStatement(statement_index))?
                .to_owned();
            let mut next_statement = statement_index + 1;
            let mut update_statement = true;

            match statement.op {
                OP_DONE => {
                    self.leave_function();
                    update_statement = false;
                }
                OP_RETURN => {
                    let a = statement.a;
                    self.copy_global(a, OFS_RETURN, 3)?;
                    self.leave_function();
                    update_statement = false;
                }
                OP_GOTO => {
                    next_statement = statement_index + statement.a as i32;
                }
                OP_IF => {
                    let cond = self.read_f32(statement.a)?;
                    if cond != 0.0 {
                        next_statement = statement_index + statement.b as i32;
                    } else {
                        next_statement = statement_index + 1;
                    }
                }
                OP_IFNOT => {
                    let cond = self.read_f32(statement.a)?;
                    if cond == 0.0 {
                        next_statement = statement_index + statement.b as i32;
                    } else {
                        next_statement = statement_index + 1;
                    }
                }
                OP_CALL0 | OP_CALL1 | OP_CALL2 | OP_CALL3 | OP_CALL4 | OP_CALL5 | OP_CALL6
                | OP_CALL7 | OP_CALL8 => {
                    let func_index = statement.a as i32;
                    let return_statement = statement_index + 1;
                    self.call_function_index(func_index, Some(return_statement))?;
                    update_statement = false;
                }
                OP_STORE_F | OP_STORE_ENT | OP_STORE_FLD | OP_STORE_S | OP_STORE_FNC => {
                    self.copy_global(statement.a, statement.b as usize, 1)?;
                }
                OP_STORE_V => {
                    self.copy_global(statement.a, statement.b as usize, 3)?;
                }
                OP_STOREP_F | OP_STOREP_ENT | OP_STOREP_FLD | OP_STOREP_S | OP_STOREP_FNC => {
                    let ptr = self.read_raw(statement.a)?;
                    self.store_pointer(ptr, statement.b, 1)?;
                }
                OP_STOREP_V => {
                    let ptr = self.read_raw(statement.a)?;
                    self.store_pointer(ptr, statement.b, 3)?;
                }
                OP_ADDRESS => {
                    let entity = self.read_ent(statement.a)? as u32;
                    let field = statement.b as u16;
                    let ptr = (entity << 16) | u32::from(field);
                    self.write_raw(statement.c, ptr)?;
                }
                OP_LOAD_F | OP_LOAD_ENT | OP_LOAD_FLD | OP_LOAD_S | OP_LOAD_FNC => {
                    let (entity, field) = self.read_entity_field(statement)?;
                    let value = self
                        .edict_field(entity, field, 1)?
                        .first()
                        .copied()
                        .unwrap_or(0);
                    self.write_raw(statement.c, value)?;
                }
                OP_LOAD_V => {
                    let (entity, field) = self.read_entity_field(statement)?;
                    let values = self.edict_field(entity, field, 3)?;
                    self.write_raw(statement.c, values[0])?;
                    self.write_raw(statement.c + 1, values[1])?;
                    self.write_raw(statement.c + 2, values[2])?;
                }
                OP_ADD_F => {
                    let value = self.read_f32(statement.a)? + self.read_f32(statement.b)?;
                    self.write_f32(statement.c, value)?;
                }
                OP_SUB_F => {
                    let value = self.read_f32(statement.a)? - self.read_f32(statement.b)?;
                    self.write_f32(statement.c, value)?;
                }
                OP_MUL_F => {
                    let value = self.read_f32(statement.a)? * self.read_f32(statement.b)?;
                    self.write_f32(statement.c, value)?;
                }
                OP_DIV_F => {
                    let denom = self.read_f32(statement.b)?;
                    let value = if denom == 0.0 {
                        0.0
                    } else {
                        self.read_f32(statement.a)? / denom
                    };
                    self.write_f32(statement.c, value)?;
                }
                OP_ADD_V => {
                    let a = self.read_vec(statement.a)?;
                    let b = self.read_vec(statement.b)?;
                    self.write_vec(statement.c, Vec3::new(a.x + b.x, a.y + b.y, a.z + b.z))?;
                }
                OP_SUB_V => {
                    let a = self.read_vec(statement.a)?;
                    let b = self.read_vec(statement.b)?;
                    self.write_vec(statement.c, Vec3::new(a.x - b.x, a.y - b.y, a.z - b.z))?;
                }
                OP_MUL_V => {
                    let a = self.read_vec(statement.a)?;
                    let b = self.read_vec(statement.b)?;
                    self.write_vec(statement.c, Vec3::new(a.x * b.x, a.y * b.y, a.z * b.z))?;
                }
                OP_MUL_FV => {
                    let scalar = self.read_f32(statement.a)?;
                    let vec = self.read_vec(statement.b)?;
                    self.write_vec(
                        statement.c,
                        Vec3::new(vec.x * scalar, vec.y * scalar, vec.z * scalar),
                    )?;
                }
                OP_MUL_VF => {
                    let vec = self.read_vec(statement.a)?;
                    let scalar = self.read_f32(statement.b)?;
                    self.write_vec(
                        statement.c,
                        Vec3::new(vec.x * scalar, vec.y * scalar, vec.z * scalar),
                    )?;
                }
                OP_EQ_F | OP_NE_F | OP_LT | OP_GT | OP_LE | OP_GE => {
                    let a = self.read_f32(statement.a)?;
                    let b = self.read_f32(statement.b)?;
                    let result = match statement.op {
                        OP_EQ_F => a == b,
                        OP_NE_F => a != b,
                        OP_LT => a < b,
                        OP_GT => a > b,
                        OP_LE => a <= b,
                        OP_GE => a >= b,
                        _ => false,
                    };
                    self.write_f32(statement.c, if result { 1.0 } else { 0.0 })?;
                }
                OP_EQ_V | OP_NE_V => {
                    let a = self.read_vec(statement.a)?;
                    let b = self.read_vec(statement.b)?;
                    let result = a == b;
                    let result = if statement.op == OP_NE_V {
                        !result
                    } else {
                        result
                    };
                    self.write_f32(statement.c, if result { 1.0 } else { 0.0 })?;
                }
                OP_EQ_S | OP_NE_S | OP_EQ_E | OP_NE_E | OP_EQ_FNC | OP_NE_FNC => {
                    let a = self.read_raw(statement.a)?;
                    let b = self.read_raw(statement.b)?;
                    let result = a == b;
                    let result = match statement.op {
                        OP_NE_S | OP_NE_E | OP_NE_FNC => !result,
                        _ => result,
                    };
                    self.write_f32(statement.c, if result { 1.0 } else { 0.0 })?;
                }
                OP_NOT_F | OP_NOT_S | OP_NOT_ENT | OP_NOT_FNC => {
                    let value = self.read_raw(statement.a)?;
                    let result = value == 0;
                    self.write_f32(statement.c, if result { 1.0 } else { 0.0 })?;
                }
                OP_NOT_V => {
                    let value = self.read_vec(statement.a)?;
                    let result = value.x == 0.0 && value.y == 0.0 && value.z == 0.0;
                    self.write_f32(statement.c, if result { 1.0 } else { 0.0 })?;
                }
                OP_AND | OP_OR => {
                    let a = self.read_f32(statement.a)?;
                    let b = self.read_f32(statement.b)?;
                    let result = if statement.op == OP_AND {
                        a != 0.0 && b != 0.0
                    } else {
                        a != 0.0 || b != 0.0
                    };
                    self.write_f32(statement.c, if result { 1.0 } else { 0.0 })?;
                }
                OP_BITAND | OP_BITOR => {
                    let a = self.read_i32(statement.a)?;
                    let b = self.read_i32(statement.b)?;
                    let result = if statement.op == OP_BITAND {
                        a & b
                    } else {
                        a | b
                    };
                    self.write_i32(statement.c, result)?;
                }
                _ => return Err(VmError::UnsupportedOpcode(statement.op)),
            }

            if update_statement && frame_index < self.call_stack.len() {
                self.call_stack[frame_index].statement_index = next_statement;
            }
        }

        Ok(())
    }

    fn enter_function(&mut self, index: usize, return_statement: Option<i32>) -> VmResult<()> {
        let func = self
            .progs
            .functions
            .get(index)
            .ok_or(VmError::BadFunction(index as i32))?;
        let first_statement = func.first_statement;
        let parm_start = func.parm_start.max(0) as usize;
        let locals = func.locals.max(0) as usize;
        let num_params = func.num_params.max(0) as usize;
        let param_sizes = func.param_sizes;

        if first_statement < 0 {
            return self.call_builtin(first_statement);
        }
        if parm_start + locals > self.globals.len() {
            return Err(VmError::BadGlobal(func.parm_start as i16));
        }

        let local_base = self.local_stack.len();
        self.local_stack
            .extend_from_slice(&self.globals[parm_start..parm_start + locals]);

        let mut dst = parm_start;
        for i in 0..num_params {
            let size = param_sizes.get(i).copied().unwrap_or(0) as usize;
            let src = OFS_PARM0 + i * PARAM_SLOT_SIZE;
            self.copy_global(src as i16, dst, size)?;
            dst += size;
        }

        self.call_stack.push(CallFrame {
            function_index: index,
            statement_index: first_statement,
            local_base,
            locals,
            return_statement,
        });

        Ok(())
    }

    fn call_builtin(&mut self, first_statement: i32) -> VmResult<()> {
        let builtin_index = (-first_statement) as usize;
        let builtin = self
            .builtins
            .get(builtin_index)
            .and_then(|entry| *entry)
            .ok_or(VmError::BuiltinNotRegistered(first_statement))?;
        builtin(self)
    }

    fn call_function_index(&mut self, index: i32, return_statement: Option<i32>) -> VmResult<()> {
        if index < 0 {
            return Err(VmError::BadFunction(index));
        }
        self.enter_function(index as usize, return_statement)
    }

    fn leave_function(&mut self) {
        let Some(frame) = self.call_stack.pop() else {
            return;
        };
        let func = match self.progs.functions.get(frame.function_index) {
            Some(func) => func,
            None => return,
        };

        let parm_start = func.parm_start.max(0) as usize;
        if parm_start + frame.locals <= self.globals.len() {
            self.globals[parm_start..parm_start + frame.locals].copy_from_slice(
                &self.local_stack[frame.local_base..frame.local_base + frame.locals],
            );
        }
        self.local_stack.truncate(frame.local_base);

        if let Some(return_statement) = frame.return_statement
            && let Some(caller) = self.call_stack.last_mut()
        {
            caller.statement_index = return_statement;
        }
    }

    fn copy_global(&mut self, src: i16, dst: usize, count: usize) -> VmResult<()> {
        if count == 0 {
            return Ok(());
        }
        let src = self.global_index(src)?;
        let end = src + count;
        let dst_end = dst + count;
        if end > self.globals.len() || dst_end > self.globals.len() {
            return Err(VmError::BadGlobal(src as i16));
        }
        let values = self.globals[src..end].to_vec();
        self.globals[dst..dst_end].copy_from_slice(&values);
        Ok(())
    }

    fn read_entity_field(&self, statement: Statement) -> VmResult<(usize, usize)> {
        let entity = self.read_ent(statement.a)?;
        let field = statement.b as i32;
        if field < 0 {
            return Err(VmError::BadField(field));
        }
        Ok((entity, field as usize))
    }

    fn read_ent(&self, ofs: i16) -> VmResult<usize> {
        let value = self.read_f32(ofs)?;
        let index = value as i32;
        if index < 0 || index as usize >= self.edicts.len() {
            return Err(VmError::BadEdict(index));
        }
        Ok(index as usize)
    }

    fn edict_field(&self, entity: usize, field: usize, size: usize) -> VmResult<Vec<u32>> {
        let edict = self
            .edicts
            .get(entity)
            .ok_or(VmError::BadEdict(entity as i32))?;
        let end = field + size;
        if end > edict.fields.len() {
            return Err(VmError::BadField(field as i32));
        }
        Ok(edict.fields[field..end].to_vec())
    }

    fn store_pointer(&mut self, ptr: u32, src: i16, count: usize) -> VmResult<()> {
        if count == 0 {
            return Ok(());
        }
        let entity = (ptr >> 16) as usize;
        let field = (ptr & 0xFFFF) as usize;
        let src_index = self.global_index(src)?;
        let src_end = src_index + count;
        let edict = self
            .edicts
            .get_mut(entity)
            .ok_or(VmError::BadEdict(entity as i32))?;
        let field_end = field + count;
        if src_end > self.globals.len() || field_end > edict.fields.len() {
            return Err(VmError::BadField(field as i32));
        }
        edict.fields[field..field_end].copy_from_slice(&self.globals[src_index..src_end]);
        Ok(())
    }

    fn global_index(&self, ofs: i16) -> VmResult<usize> {
        if ofs < 0 {
            return Err(VmError::BadGlobal(ofs));
        }
        let index = ofs as usize;
        if index >= self.globals.len() {
            return Err(VmError::BadGlobal(ofs));
        }
        Ok(index)
    }

    fn read_raw(&self, ofs: i16) -> VmResult<u32> {
        let index = self.global_index(ofs)?;
        Ok(self.globals[index])
    }

    fn write_raw(&mut self, ofs: i16, value: u32) -> VmResult<()> {
        let index = self.global_index(ofs)?;
        self.globals[index] = value;
        Ok(())
    }

    fn read_f32(&self, ofs: i16) -> VmResult<f32> {
        Ok(f32::from_bits(self.read_raw(ofs)?))
    }

    fn write_f32(&mut self, ofs: i16, value: f32) -> VmResult<()> {
        self.write_raw(ofs, value.to_bits())
    }

    fn read_i32(&self, ofs: i16) -> VmResult<i32> {
        Ok(self.read_f32(ofs)?.trunc() as i32)
    }

    fn write_i32(&mut self, ofs: i16, value: i32) -> VmResult<()> {
        self.write_f32(ofs, value as f32)
    }

    fn read_vec(&self, ofs: i16) -> VmResult<Vec3> {
        let x = self.read_f32(ofs)?;
        let y = self.read_f32(ofs + 1)?;
        let z = self.read_f32(ofs + 2)?;
        Ok(Vec3::new(x, y, z))
    }

    fn write_vec(&mut self, ofs: i16, value: Vec3) -> VmResult<()> {
        self.write_f32(ofs, value.x)?;
        self.write_f32(ofs + 1, value.y)?;
        self.write_f32(ofs + 2, value.z)
    }
}

const OP_DONE: u16 = 0;
const OP_MUL_F: u16 = 1;
const OP_MUL_V: u16 = 2;
const OP_MUL_FV: u16 = 3;
const OP_MUL_VF: u16 = 4;
const OP_DIV_F: u16 = 5;
const OP_ADD_F: u16 = 6;
const OP_ADD_V: u16 = 7;
const OP_SUB_F: u16 = 8;
const OP_SUB_V: u16 = 9;
const OP_EQ_F: u16 = 10;
const OP_EQ_V: u16 = 11;
const OP_EQ_S: u16 = 12;
const OP_EQ_E: u16 = 13;
const OP_EQ_FNC: u16 = 14;
const OP_NE_F: u16 = 15;
const OP_NE_V: u16 = 16;
const OP_NE_S: u16 = 17;
const OP_NE_E: u16 = 18;
const OP_NE_FNC: u16 = 19;
const OP_LE: u16 = 20;
const OP_GE: u16 = 21;
const OP_LT: u16 = 22;
const OP_GT: u16 = 23;
const OP_LOAD_F: u16 = 24;
const OP_LOAD_V: u16 = 25;
const OP_LOAD_S: u16 = 26;
const OP_LOAD_ENT: u16 = 27;
const OP_LOAD_FLD: u16 = 28;
const OP_LOAD_FNC: u16 = 29;
const OP_ADDRESS: u16 = 30;
const OP_STORE_F: u16 = 31;
const OP_STORE_V: u16 = 32;
const OP_STORE_S: u16 = 33;
const OP_STORE_ENT: u16 = 34;
const OP_STORE_FLD: u16 = 35;
const OP_STORE_FNC: u16 = 36;
const OP_STOREP_F: u16 = 37;
const OP_STOREP_V: u16 = 38;
const OP_STOREP_S: u16 = 39;
const OP_STOREP_ENT: u16 = 40;
const OP_STOREP_FLD: u16 = 41;
const OP_STOREP_FNC: u16 = 42;
const OP_RETURN: u16 = 43;
const OP_NOT_F: u16 = 44;
const OP_NOT_V: u16 = 45;
const OP_NOT_S: u16 = 46;
const OP_NOT_ENT: u16 = 47;
const OP_NOT_FNC: u16 = 48;
const OP_IF: u16 = 49;
const OP_IFNOT: u16 = 50;
const OP_CALL0: u16 = 51;
const OP_CALL1: u16 = 52;
const OP_CALL2: u16 = 53;
const OP_CALL3: u16 = 54;
const OP_CALL4: u16 = 55;
const OP_CALL5: u16 = 56;
const OP_CALL6: u16 = 57;
const OP_CALL7: u16 = 58;
const OP_CALL8: u16 = 59;
#[allow(dead_code)]
const OP_STATE: u16 = 60;
const OP_GOTO: u16 = 61;
const OP_AND: u16 = 62;
const OP_OR: u16 = 63;
const OP_BITAND: u16 = 64;
const OP_BITOR: u16 = 65;

fn read_defs(bytes: &[u8], lump: Lump, strings: &[u8]) -> Result<Vec<Definition>, ProgsError> {
    let size = lump
        .count
        .try_into()
        .ok()
        .and_then(|count: usize| count.checked_mul(8))
        .ok_or(ProgsError::InvalidLump("defs"))?;
    let data = read_bytes(bytes, lump, "defs", size)?;
    let mut defs = Vec::new();
    for idx in 0..lump.count as usize {
        let base = idx * 8;
        let raw_type = read_i16(&data, base)?;
        let offset = read_i16(&data, base + 2)?;
        let name_offset = read_i32(&data, base + 4)?;
        let save_global = (raw_type & DEF_SAVEGLOBAL) != 0;
        let ty = qc_type(raw_type & !DEF_SAVEGLOBAL);
        let name = read_cstring(strings, name_offset)?;
        defs.push(Definition {
            ty,
            offset,
            name,
            save_global,
        });
    }
    Ok(defs)
}

fn read_functions(bytes: &[u8], lump: Lump, strings: &[u8]) -> Result<Vec<Function>, ProgsError> {
    let size = lump
        .count
        .try_into()
        .ok()
        .and_then(|count: usize| count.checked_mul(36))
        .ok_or(ProgsError::InvalidLump("functions"))?;
    let data = read_bytes(bytes, lump, "functions", size)?;
    let mut functions = Vec::new();
    for idx in 0..lump.count as usize {
        let base = idx * 36;
        let first_statement = read_i32(&data, base)?;
        let parm_start = read_i32(&data, base + 4)?;
        let locals = read_i32(&data, base + 8)?;
        let profile = read_i32(&data, base + 12)?;
        let name_offset = read_i32(&data, base + 16)?;
        let file_offset = read_i32(&data, base + 20)?;
        let num_params = read_i32(&data, base + 24)?;
        let mut param_sizes = [0u8; 8];
        param_sizes.copy_from_slice(&data[base + 28..base + 36]);
        let name = read_cstring(strings, name_offset)?;
        let file = read_cstring(strings, file_offset)?;
        functions.push(Function {
            first_statement,
            parm_start,
            locals,
            profile,
            name,
            file,
            num_params,
            param_sizes,
        });
    }
    Ok(functions)
}

fn read_statements(bytes: &[u8], lump: Lump) -> Result<Vec<Statement>, ProgsError> {
    let size = lump
        .count
        .try_into()
        .ok()
        .and_then(|count: usize| count.checked_mul(8))
        .ok_or(ProgsError::InvalidLump("statements"))?;
    let data = read_bytes(bytes, lump, "statements", size)?;
    let mut statements = Vec::new();
    for idx in 0..lump.count as usize {
        let base = idx * 8;
        let op = read_u16(&data, base)?;
        let a = read_i16(&data, base + 2)?;
        let b = read_i16(&data, base + 4)?;
        let c = read_i16(&data, base + 6)?;
        statements.push(Statement { op, a, b, c });
    }
    Ok(statements)
}

fn read_globals(bytes: &[u8], lump: Lump) -> Result<Vec<u32>, ProgsError> {
    let size = lump
        .count
        .try_into()
        .ok()
        .and_then(|count: usize| count.checked_mul(4))
        .ok_or(ProgsError::InvalidLump("globals"))?;
    let data = read_bytes(bytes, lump, "globals", size)?;
    let mut globals = Vec::new();
    for idx in 0..lump.count as usize {
        let base = idx * 4;
        let value = read_u32(&data, base)?;
        globals.push(value);
    }
    Ok(globals)
}

#[derive(Clone, Copy)]
struct Lump {
    offset: i32,
    count: i32,
}

fn read_lump(bytes: &[u8], offset: usize) -> Result<Lump, ProgsError> {
    Ok(Lump {
        offset: read_i32(bytes, offset)?,
        count: read_i32(bytes, offset + 4)?,
    })
}

fn read_bytes(
    bytes: &[u8],
    lump: Lump,
    name: &'static str,
    byte_len: usize,
) -> Result<Vec<u8>, ProgsError> {
    let offset = lump.offset;
    if offset < 0 || lump.count < 0 {
        return Err(ProgsError::InvalidLump(name));
    }
    let start = offset as usize;
    let end = start
        .checked_add(byte_len)
        .ok_or(ProgsError::InvalidLump(name))?;
    if end > bytes.len() {
        return Err(ProgsError::InvalidLump(name));
    }
    Ok(bytes[start..end].to_vec())
}

fn read_cstring(bytes: &[u8], offset: i32) -> Result<String, ProgsError> {
    if offset <= 0 {
        return Ok(String::new());
    }
    let start = offset as usize;
    if start >= bytes.len() {
        return Err(ProgsError::InvalidStringOffset(offset));
    }
    let end = bytes[start..]
        .iter()
        .position(|&b| b == 0)
        .map(|idx| start + idx)
        .unwrap_or(bytes.len());
    std::str::from_utf8(&bytes[start..end])
        .map(|value| value.to_string())
        .map_err(|_| ProgsError::InvalidUtf8)
}

fn qc_type(raw: i16) -> QcType {
    match raw {
        0 => QcType::Void,
        1 => QcType::String,
        2 => QcType::Float,
        3 => QcType::Vector,
        4 => QcType::Entity,
        5 => QcType::Field,
        6 => QcType::Function,
        7 => QcType::Pointer,
        8 => QcType::Integer,
        other => QcType::Unknown(other),
    }
}

fn read_i32(bytes: &[u8], offset: usize) -> Result<i32, ProgsError> {
    let slice = bytes
        .get(offset..offset + 4)
        .ok_or(ProgsError::BufferTooSmall)?;
    Ok(i32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, ProgsError> {
    let slice = bytes
        .get(offset..offset + 4)
        .ok_or(ProgsError::BufferTooSmall)?;
    Ok(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn read_i16(bytes: &[u8], offset: usize) -> Result<i16, ProgsError> {
    let slice = bytes
        .get(offset..offset + 2)
        .ok_or(ProgsError::BufferTooSmall)?;
    Ok(i16::from_le_bytes([slice[0], slice[1]]))
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, ProgsError> {
    let slice = bytes
        .get(offset..offset + 2)
        .ok_or(ProgsError::BufferTooSmall)?;
    Ok(u16::from_le_bytes([slice[0], slice[1]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn push_i32(out: &mut Vec<u8>, value: i32) {
        out.extend_from_slice(&value.to_le_bytes());
    }

    fn push_i16(out: &mut Vec<u8>, value: i16) {
        out.extend_from_slice(&value.to_le_bytes());
    }

    fn push_u16(out: &mut Vec<u8>, value: u16) {
        out.extend_from_slice(&value.to_le_bytes());
    }

    #[test]
    fn parses_minimal_progs() {
        let mut bytes = Vec::new();

        let header_size = 60i32;
        let statements_offset = header_size;
        let statements_count = 1i32;
        let functions_offset = statements_offset + statements_count * 8;
        let functions_count = 1i32;
        let strings_offset = functions_offset + functions_count * 36;
        let strings_data = b"\0main\0".to_vec();
        let strings_count = strings_data.len() as i32;
        let globals_offset = strings_offset + strings_count;
        let globals_count = 8i32;

        push_i32(&mut bytes, PROG_VERSION);
        push_i32(&mut bytes, 0);
        push_i32(&mut bytes, statements_offset);
        push_i32(&mut bytes, statements_count);
        push_i32(&mut bytes, 0);
        push_i32(&mut bytes, 0);
        push_i32(&mut bytes, 0);
        push_i32(&mut bytes, 0);
        push_i32(&mut bytes, functions_offset);
        push_i32(&mut bytes, functions_count);
        push_i32(&mut bytes, strings_offset);
        push_i32(&mut bytes, strings_count);
        push_i32(&mut bytes, globals_offset);
        push_i32(&mut bytes, globals_count);
        push_i32(&mut bytes, 0);

        while bytes.len() < statements_offset as usize {
            bytes.push(0);
        }
        push_u16(&mut bytes, OP_DONE);
        push_i16(&mut bytes, 0);
        push_i16(&mut bytes, 0);
        push_i16(&mut bytes, 0);

        while bytes.len() < functions_offset as usize {
            bytes.push(0);
        }
        push_i32(&mut bytes, 0);
        push_i32(&mut bytes, 0);
        push_i32(&mut bytes, 0);
        push_i32(&mut bytes, 0);
        push_i32(&mut bytes, 1);
        push_i32(&mut bytes, 0);
        push_i32(&mut bytes, 0);
        bytes.extend_from_slice(&[0u8; 8]);

        while bytes.len() < strings_offset as usize {
            bytes.push(0);
        }
        bytes.extend_from_slice(&strings_data);

        while bytes.len() < globals_offset as usize {
            bytes.push(0);
        }
        for _ in 0..globals_count {
            push_i32(&mut bytes, 0);
        }

        let progs = ProgsDat::from_bytes(&bytes).unwrap();
        assert_eq!(progs.version, PROG_VERSION);
        assert_eq!(progs.statements.len(), 1);
        assert_eq!(progs.functions.len(), 1);
        assert_eq!(progs.functions[0].name, "main");
    }

    #[test]
    fn reads_strings() {
        let bytes = b"\0hello\0world\0";
        assert_eq!(read_cstring(bytes, 1).unwrap(), "hello");
        assert_eq!(read_cstring(bytes, 7).unwrap(), "world");
        assert_eq!(read_cstring(bytes, 0).unwrap(), "");
    }

    #[test]
    fn vm_allocates_edicts_and_reads_globals() {
        let progs = ProgsDat {
            version: PROG_VERSION,
            crc: 0,
            statements: Vec::new(),
            global_defs: Vec::new(),
            field_defs: Vec::new(),
            functions: Vec::new(),
            strings: Vec::new(),
            globals: vec![0; 16],
            entity_fields: 6,
        };

        let mut vm = Vm::new(progs);
        assert_eq!(vm.edict_count(), 1);
        let entity = vm.alloc_edict();
        assert_eq!(entity, 1);
        assert_eq!(vm.edict_count(), 2);

        vm.write_global_f32(0, 2.5).unwrap();
        assert_eq!(vm.read_global_f32(0).unwrap(), 2.5);

        let vec = Vec3::new(1.0, 2.0, 3.0);
        vm.write_global_vec(1, vec).unwrap();
        assert_eq!(vm.read_global_vec(1).unwrap(), vec);

        vm.write_edict_field_f32(entity, 0, 4.0).unwrap();
        assert_eq!(vm.read_edict_field_f32(entity, 0).unwrap(), 4.0);

        vm.write_edict_field_vec(entity, 1, vec).unwrap();
        assert_eq!(vm.read_edict_field_vec(entity, 1).unwrap(), vec);
    }
}
