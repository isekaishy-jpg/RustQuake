use qw_common::{Entity, Vec3};
use qw_qc::{QcType, Vm, VmError};
use std::collections::HashMap;

#[derive(Default)]
pub struct ServerQcContext {
    precache_files: Vec<String>,
    precache_models: Vec<String>,
    precache_sounds: Vec<String>,
    cvars: HashMap<String, String>,
    prints: Vec<String>,
    rng_state: u32,
    globals: QcGlobals,
    fields: QcFields,
}

#[derive(Default, Clone, Copy)]
struct QcGlobals {
    self_ofs: Option<i16>,
    other_ofs: Option<i16>,
    world_ofs: Option<i16>,
    time_ofs: Option<i16>,
    frametime_ofs: Option<i16>,
    mapname_ofs: Option<i16>,
    v_forward_ofs: Option<i16>,
    v_right_ofs: Option<i16>,
    v_up_ofs: Option<i16>,
    trace_allsolid_ofs: Option<i16>,
    trace_startsolid_ofs: Option<i16>,
    trace_fraction_ofs: Option<i16>,
    trace_endpos_ofs: Option<i16>,
    trace_plane_normal_ofs: Option<i16>,
    trace_plane_dist_ofs: Option<i16>,
    trace_ent_ofs: Option<i16>,
    trace_inopen_ofs: Option<i16>,
    trace_inwater_ofs: Option<i16>,
}

#[derive(Default, Clone, Copy)]
struct QcFields {
    origin: Option<usize>,
    mins: Option<usize>,
    maxs: Option<usize>,
    size: Option<usize>,
    absmin: Option<usize>,
    absmax: Option<usize>,
    model: Option<usize>,
    classname: Option<usize>,
}

pub fn configure_vm(vm: &mut Vm, mapname: &str) -> Result<(), VmError> {
    if vm.context_ref::<ServerQcContext>().is_none() {
        vm.set_context(ServerQcContext::default());
    }

    let globals = resolve_globals(vm);
    let fields = resolve_fields(vm);

    if let Some(ctx) = vm.context_mut::<ServerQcContext>() {
        ctx.globals = globals;
        ctx.fields = fields;
    }

    init_globals(vm, mapname)?;
    register_builtins(vm);
    Ok(())
}

pub fn apply_worldspawn(vm: &mut Vm, entities: &[Entity]) -> Result<(), VmError> {
    let Some(worldspawn) = entities
        .iter()
        .find(|entity| entity.get("classname") == Some("worldspawn"))
    else {
        return Ok(());
    };
    apply_entity_pairs(vm, 0, worldspawn)?;
    Ok(())
}

pub fn spawn_entities(vm: &mut Vm, entities: &[Entity]) -> Result<(), VmError> {
    let globals = vm
        .context_ref::<ServerQcContext>()
        .map(|ctx| ctx.globals)
        .unwrap_or_default();
    let Some(self_ofs) = globals.self_ofs else {
        return Ok(());
    };

    for entity in entities {
        let Some(classname) = entity.get("classname") else {
            continue;
        };
        if classname.eq_ignore_ascii_case("worldspawn") {
            continue;
        }

        let ent = vm.alloc_edict();
        apply_entity_pairs(vm, ent, entity)?;
        vm.write_global_f32(self_ofs, ent as f32)?;

        if let Some(func) = vm.progs().function_index(classname) {
            vm.call_function(func, 10_000)?;
        } else {
            println!("[server] missing spawn function for {classname}");
        }
    }

    Ok(())
}

fn resolve_globals(vm: &Vm) -> QcGlobals {
    QcGlobals {
        self_ofs: global_offset(vm, "self"),
        other_ofs: global_offset(vm, "other"),
        world_ofs: global_offset(vm, "world"),
        time_ofs: global_offset(vm, "time"),
        frametime_ofs: global_offset(vm, "frametime"),
        mapname_ofs: global_offset(vm, "mapname"),
        v_forward_ofs: global_offset(vm, "v_forward"),
        v_right_ofs: global_offset(vm, "v_right"),
        v_up_ofs: global_offset(vm, "v_up"),
        trace_allsolid_ofs: global_offset(vm, "trace_allsolid"),
        trace_startsolid_ofs: global_offset(vm, "trace_startsolid"),
        trace_fraction_ofs: global_offset(vm, "trace_fraction"),
        trace_endpos_ofs: global_offset(vm, "trace_endpos"),
        trace_plane_normal_ofs: global_offset(vm, "trace_plane_normal"),
        trace_plane_dist_ofs: global_offset(vm, "trace_plane_dist"),
        trace_ent_ofs: global_offset(vm, "trace_ent"),
        trace_inopen_ofs: global_offset(vm, "trace_inopen"),
        trace_inwater_ofs: global_offset(vm, "trace_inwater"),
    }
}

fn resolve_fields(vm: &Vm) -> QcFields {
    QcFields {
        origin: field_offset(vm, "origin"),
        mins: field_offset(vm, "mins"),
        maxs: field_offset(vm, "maxs"),
        size: field_offset(vm, "size"),
        absmin: field_offset(vm, "absmin"),
        absmax: field_offset(vm, "absmax"),
        model: field_offset(vm, "model"),
        classname: field_offset(vm, "classname"),
    }
}

fn init_globals(vm: &mut Vm, mapname: &str) -> Result<(), VmError> {
    let globals = vm
        .context_ref::<ServerQcContext>()
        .map(|ctx| ctx.globals)
        .unwrap_or_default();

    if let Some(ofs) = globals.self_ofs {
        vm.write_global_f32(ofs, 0.0)?;
    }
    if let Some(ofs) = globals.other_ofs {
        vm.write_global_f32(ofs, 0.0)?;
    }
    if let Some(ofs) = globals.world_ofs {
        vm.write_global_f32(ofs, 0.0)?;
    }
    if let Some(ofs) = globals.time_ofs {
        vm.write_global_f32(ofs, 0.0)?;
    }
    if let Some(ofs) = globals.frametime_ofs {
        vm.write_global_f32(ofs, 0.0)?;
    }
    if let Some(ofs) = globals.mapname_ofs {
        let offset = vm.alloc_string(mapname)?;
        vm.write_global_raw(ofs, offset as u32)?;
    }
    Ok(())
}

fn register_builtins(vm: &mut Vm) {
    let mut builtin_map = HashMap::new();
    for func in &vm.progs().functions {
        if func.first_statement < 0 {
            let index = (-func.first_statement) as usize;
            builtin_map
                .entry(index)
                .or_insert_with(|| func.name.clone());
        }
    }

    for (index, name) in builtin_map {
        let name = name.to_ascii_lowercase();
        let builtin = match name.as_str() {
            "dprint" => builtin_dprint,
            "bprint" => builtin_bprint,
            "sprint" => builtin_sprint,
            "centerprint" => builtin_centerprint,
            "precache_file" | "precache_file2" => builtin_precache_file,
            "precache_model" => builtin_precache_model,
            "precache_sound" => builtin_precache_sound,
            "random" => builtin_random,
            "ftos" => builtin_ftos,
            "vtos" => builtin_vtos,
            "stof" => builtin_stof,
            "cvar" => builtin_cvar,
            "cvar_set" => builtin_cvar_set,
            "makevectors" => builtin_makevectors,
            "setorigin" => builtin_setorigin,
            "setsize" => builtin_setsize,
            "setmodel" => builtin_setmodel,
            "spawn" => builtin_spawn,
            "remove" => builtin_remove,
            "find" => builtin_find,
            "nextent" => builtin_nextent,
            "traceline" => builtin_traceline,
            "vlen" => builtin_vlen,
            "normalize" => builtin_normalize,
            "vectoyaw" => builtin_vectoyaw,
            "vectoangles" => builtin_vectoangles,
            "fabs" => builtin_fabs,
            "rint" => builtin_rint,
            "floor" => builtin_floor,
            "ceil" => builtin_ceil,
            "setspawnparms" => builtin_noop,
            _ => builtin_noop,
        };

        vm.register_builtin(index, builtin);
    }
}

fn builtin_noop(vm: &mut Vm) -> Result<(), VmError> {
    vm.set_return_f32(0.0)
}

fn builtin_dprint(vm: &mut Vm) -> Result<(), VmError> {
    let message = read_param_string(vm, 0);
    if let Some(ctx) = vm.context_mut::<ServerQcContext>() {
        ctx.prints.push(message.clone());
    }
    println!("[qc] {message}");
    Ok(())
}

fn builtin_bprint(vm: &mut Vm) -> Result<(), VmError> {
    builtin_dprint(vm)
}

fn builtin_sprint(vm: &mut Vm) -> Result<(), VmError> {
    let message = read_param_string(vm, 1);
    if let Some(ctx) = vm.context_mut::<ServerQcContext>() {
        ctx.prints.push(message.clone());
    }
    println!("[qc] {message}");
    Ok(())
}

fn builtin_centerprint(vm: &mut Vm) -> Result<(), VmError> {
    builtin_sprint(vm)
}

fn builtin_precache_file(vm: &mut Vm) -> Result<(), VmError> {
    let value = read_param_string(vm, 0);
    if let Some(ctx) = vm.context_mut::<ServerQcContext>() {
        push_unique(&mut ctx.precache_files, value);
    }
    Ok(())
}

fn builtin_precache_model(vm: &mut Vm) -> Result<(), VmError> {
    let value = read_param_string(vm, 0);
    if let Some(ctx) = vm.context_mut::<ServerQcContext>() {
        push_unique(&mut ctx.precache_models, value);
    }
    Ok(())
}

fn builtin_precache_sound(vm: &mut Vm) -> Result<(), VmError> {
    let value = read_param_string(vm, 0);
    if let Some(ctx) = vm.context_mut::<ServerQcContext>() {
        push_unique(&mut ctx.precache_sounds, value);
    }
    Ok(())
}

fn builtin_random(vm: &mut Vm) -> Result<(), VmError> {
    let value = if let Some(ctx) = vm.context_mut::<ServerQcContext>() {
        ctx.rng_state = ctx.rng_state.wrapping_mul(1664525).wrapping_add(1013904223);
        ctx.rng_state as f32 / u32::MAX as f32
    } else {
        0.0
    };
    vm.set_return_f32(value)
}

fn builtin_ftos(vm: &mut Vm) -> Result<(), VmError> {
    let value = vm.read_param_f32(0)?;
    vm.set_return_string(&format!("{value}"))
}

fn builtin_vtos(vm: &mut Vm) -> Result<(), VmError> {
    let value = vm.read_param_vec(0)?;
    vm.set_return_string(&format!("{} {} {}", value.x, value.y, value.z))
}

fn builtin_stof(vm: &mut Vm) -> Result<(), VmError> {
    let value = read_param_string(vm, 0);
    let parsed = value.trim().parse::<f32>().unwrap_or(0.0);
    vm.set_return_f32(parsed)
}

fn builtin_cvar(vm: &mut Vm) -> Result<(), VmError> {
    let name = read_param_string(vm, 0);
    let value = vm
        .context_ref::<ServerQcContext>()
        .and_then(|ctx| ctx.cvars.get(&name))
        .and_then(|value| value.parse::<f32>().ok())
        .unwrap_or(0.0);
    vm.set_return_f32(value)
}

fn builtin_cvar_set(vm: &mut Vm) -> Result<(), VmError> {
    let name = read_param_string(vm, 0);
    let value = read_param_string(vm, 1);
    if let Some(ctx) = vm.context_mut::<ServerQcContext>() {
        ctx.cvars.insert(name, value);
    }
    Ok(())
}

fn builtin_makevectors(vm: &mut Vm) -> Result<(), VmError> {
    let angles = vm.read_param_vec(0)?;
    let (forward, right, up) = angle_vectors(angles);
    let globals = vm
        .context_ref::<ServerQcContext>()
        .map(|ctx| ctx.globals)
        .unwrap_or_default();

    if let Some(ofs) = globals.v_forward_ofs {
        vm.write_global_vec(ofs, forward)?;
    }
    if let Some(ofs) = globals.v_right_ofs {
        vm.write_global_vec(ofs, right)?;
    }
    if let Some(ofs) = globals.v_up_ofs {
        vm.write_global_vec(ofs, up)?;
    }
    Ok(())
}

fn builtin_setorigin(vm: &mut Vm) -> Result<(), VmError> {
    let ent = read_param_entity(vm, 0)?;
    let origin = vm.read_param_vec(1)?;
    let fields = fields_from_context(vm);

    if let Some(ofs) = fields.origin {
        vm.write_edict_field_vec(ent, ofs, origin)?;
    }

    update_abs_bounds(vm, ent, fields)?;
    Ok(())
}

fn builtin_setsize(vm: &mut Vm) -> Result<(), VmError> {
    let ent = read_param_entity(vm, 0)?;
    let mins = vm.read_param_vec(1)?;
    let maxs = vm.read_param_vec(2)?;
    let fields = fields_from_context(vm);

    if let Some(ofs) = fields.mins {
        vm.write_edict_field_vec(ent, ofs, mins)?;
    }
    if let Some(ofs) = fields.maxs {
        vm.write_edict_field_vec(ent, ofs, maxs)?;
    }
    if let Some(ofs) = fields.size {
        vm.write_edict_field_vec(ent, ofs, vec_sub(maxs, mins))?;
    }

    update_abs_bounds(vm, ent, fields)?;
    Ok(())
}

fn builtin_setmodel(vm: &mut Vm) -> Result<(), VmError> {
    let ent = read_param_entity(vm, 0)?;
    let model = vm.read_param_raw(1)?;
    let fields = fields_from_context(vm);
    if let Some(ofs) = fields.model {
        vm.write_edict_field_raw(ent, ofs, &[model])?;
    }
    Ok(())
}

fn builtin_spawn(vm: &mut Vm) -> Result<(), VmError> {
    let ent = vm.alloc_edict();
    vm.set_return_f32(ent as f32)
}

fn builtin_remove(vm: &mut Vm) -> Result<(), VmError> {
    let ent = read_param_entity(vm, 0)?;
    let field_count = vm.edict_field_count();
    let zeros = vec![0u32; field_count];
    vm.write_edict_field_raw(ent, 0, &zeros)
}

fn builtin_find(vm: &mut Vm) -> Result<(), VmError> {
    let start = vm.read_param_f32(0)? as i32;
    let field = vm.read_param_raw(1)? as usize;
    let target = read_param_string(vm, 2);

    let mut index = if start < 0 { 0 } else { start + 1 } as usize;
    while index < vm.edict_count() {
        if let Some(value) = read_edict_string(vm, index, field) {
            if value.eq_ignore_ascii_case(&target) {
                return vm.set_return_f32(index as f32);
            }
        }
        index += 1;
    }

    vm.set_return_f32(0.0)
}

fn builtin_nextent(vm: &mut Vm) -> Result<(), VmError> {
    let start = vm.read_param_f32(0)? as i32;
    let mut index = if start < 0 { 0 } else { start + 1 } as usize;
    let fields = fields_from_context(vm);

    while index < vm.edict_count() {
        if let Some(classname_ofs) = fields.classname {
            if let Some(value) = read_edict_string(vm, index, classname_ofs) {
                if !value.is_empty() {
                    return vm.set_return_f32(index as f32);
                }
            }
        } else {
            return vm.set_return_f32(index as f32);
        }
        index += 1;
    }

    vm.set_return_f32(0.0)
}

fn builtin_traceline(vm: &mut Vm) -> Result<(), VmError> {
    let end = vm.read_param_vec(1)?;
    let globals = vm
        .context_ref::<ServerQcContext>()
        .map(|ctx| ctx.globals)
        .unwrap_or_default();

    if let Some(ofs) = globals.trace_allsolid_ofs {
        vm.write_global_f32(ofs, 0.0)?;
    }
    if let Some(ofs) = globals.trace_startsolid_ofs {
        vm.write_global_f32(ofs, 0.0)?;
    }
    if let Some(ofs) = globals.trace_fraction_ofs {
        vm.write_global_f32(ofs, 1.0)?;
    }
    if let Some(ofs) = globals.trace_endpos_ofs {
        vm.write_global_vec(ofs, end)?;
    }
    if let Some(ofs) = globals.trace_plane_normal_ofs {
        vm.write_global_vec(ofs, Vec3::default())?;
    }
    if let Some(ofs) = globals.trace_plane_dist_ofs {
        vm.write_global_f32(ofs, 0.0)?;
    }
    if let Some(ofs) = globals.trace_ent_ofs {
        vm.write_global_f32(ofs, 0.0)?;
    }
    if let Some(ofs) = globals.trace_inopen_ofs {
        vm.write_global_f32(ofs, 0.0)?;
    }
    if let Some(ofs) = globals.trace_inwater_ofs {
        vm.write_global_f32(ofs, 0.0)?;
    }

    vm.set_return_f32(1.0)
}

fn builtin_vlen(vm: &mut Vm) -> Result<(), VmError> {
    let value = vm.read_param_vec(0)?;
    let len = (value.x * value.x + value.y * value.y + value.z * value.z).sqrt();
    vm.set_return_f32(len)
}

fn builtin_normalize(vm: &mut Vm) -> Result<(), VmError> {
    let value = vm.read_param_vec(0)?;
    let len = (value.x * value.x + value.y * value.y + value.z * value.z).sqrt();
    let normalized = if len == 0.0 {
        Vec3::default()
    } else {
        vec_scale(value, 1.0 / len)
    };
    vm.set_return_vec(normalized)
}

fn builtin_vectoyaw(vm: &mut Vm) -> Result<(), VmError> {
    let value = vm.read_param_vec(0)?;
    let yaw = value.y.atan2(value.x).to_degrees();
    vm.set_return_f32(yaw)
}

fn builtin_vectoangles(vm: &mut Vm) -> Result<(), VmError> {
    let value = vm.read_param_vec(0)?;
    let yaw = value.y.atan2(value.x).to_degrees();
    let forward = (value.x * value.x + value.y * value.y).sqrt();
    let pitch = (-value.z).atan2(forward).to_degrees();
    vm.set_return_vec(Vec3::new(pitch, yaw, 0.0))
}

fn builtin_fabs(vm: &mut Vm) -> Result<(), VmError> {
    let value = vm.read_param_f32(0)?.abs();
    vm.set_return_f32(value)
}

fn builtin_rint(vm: &mut Vm) -> Result<(), VmError> {
    let value = vm.read_param_f32(0)?.round();
    vm.set_return_f32(value)
}

fn builtin_floor(vm: &mut Vm) -> Result<(), VmError> {
    let value = vm.read_param_f32(0)?.floor();
    vm.set_return_f32(value)
}

fn builtin_ceil(vm: &mut Vm) -> Result<(), VmError> {
    let value = vm.read_param_f32(0)?.ceil();
    vm.set_return_f32(value)
}

fn read_param_entity(vm: &mut Vm, param: usize) -> Result<usize, VmError> {
    let value = vm.read_param_f32(param)?;
    if value < 0.0 {
        return Err(VmError::BadEdict(value as i32));
    }
    let index = value as usize;
    if index >= vm.edict_count() {
        return Err(VmError::BadEdict(index as i32));
    }
    Ok(index)
}

fn read_param_string(vm: &mut Vm, param: usize) -> String {
    vm.read_param_string(param).unwrap_or_default()
}

fn global_offset(vm: &Vm, name: &str) -> Option<i16> {
    vm.global_def(name).map(|def| def.offset)
}

fn field_offset(vm: &Vm, name: &str) -> Option<usize> {
    vm.field_def(name).and_then(|def| {
        if def.offset < 0 {
            None
        } else {
            Some(def.offset as usize)
        }
    })
}

fn fields_from_context(vm: &Vm) -> QcFields {
    vm.context_ref::<ServerQcContext>()
        .map(|ctx| ctx.fields)
        .unwrap_or_default()
}

fn read_edict_string(vm: &Vm, ent: usize, field: usize) -> Option<String> {
    let value = vm
        .read_edict_field_raw(ent, field, 1)
        .ok()?
        .first()
        .copied()
        .unwrap_or(0);
    vm.progs().string_at(value as i32).ok()
}

fn update_abs_bounds(vm: &mut Vm, ent: usize, fields: QcFields) -> Result<(), VmError> {
    let (Some(origin_ofs), Some(mins_ofs), Some(maxs_ofs), Some(absmin_ofs), Some(absmax_ofs)) = (
        fields.origin,
        fields.mins,
        fields.maxs,
        fields.absmin,
        fields.absmax,
    ) else {
        return Ok(());
    };

    let origin = vm.read_edict_field_vec(ent, origin_ofs)?;
    let mins = vm.read_edict_field_vec(ent, mins_ofs)?;
    let maxs = vm.read_edict_field_vec(ent, maxs_ofs)?;
    vm.write_edict_field_vec(ent, absmin_ofs, vec_add(origin, mins))?;
    vm.write_edict_field_vec(ent, absmax_ofs, vec_add(origin, maxs))?;
    Ok(())
}

fn angle_vectors(angles: Vec3) -> (Vec3, Vec3, Vec3) {
    let (pitch, yaw, roll) = (
        angles.x.to_radians(),
        angles.y.to_radians(),
        angles.z.to_radians(),
    );
    let (sp, cp) = pitch.sin_cos();
    let (sy, cy) = yaw.sin_cos();
    let (sr, cr) = roll.sin_cos();

    let forward = Vec3::new(cp * cy, cp * sy, -sp);
    let right = Vec3::new(-sr * sp * cy + cr * sy, -sr * sp * sy - cr * cy, -sr * cp);
    let up = Vec3::new(cr * sp * cy + sr * sy, cr * sp * sy - sr * cy, cr * cp);
    (forward, right, up)
}

fn vec_add(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(a.x + b.x, a.y + b.y, a.z + b.z)
}

fn vec_sub(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(a.x - b.x, a.y - b.y, a.z - b.z)
}

fn vec_scale(a: Vec3, scale: f32) -> Vec3 {
    Vec3::new(a.x * scale, a.y * scale, a.z * scale)
}

fn apply_entity_pairs(vm: &mut Vm, ent: usize, entity: &Entity) -> Result<(), VmError> {
    for (key, value) in entity.pairs() {
        if key.starts_with('_') {
            continue;
        }

        if key.eq_ignore_ascii_case("angle") {
            apply_angle_field(vm, ent, value);
            continue;
        }

        let Some(def) = vm.field_def(key) else {
            continue;
        };
        if def.offset < 0 {
            continue;
        }
        let field = def.offset as usize;

        match def.ty {
            QcType::String => {
                let offset = vm.alloc_string(value)?;
                vm.write_edict_field_raw(ent, field, &[offset as u32])?;
            }
            QcType::Float | QcType::Integer | QcType::Entity => {
                if let Ok(parsed) = value.trim().parse::<f32>() {
                    vm.write_edict_field_f32(ent, field, parsed)?;
                }
            }
            QcType::Vector => {
                if let Some(vec) = parse_vec3(value) {
                    vm.write_edict_field_vec(ent, field, vec)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn apply_angle_field(vm: &mut Vm, ent: usize, value: &str) {
    let Some(def) = vm.field_def("angles") else {
        return;
    };
    if def.offset < 0 {
        return;
    }
    let field = def.offset as usize;
    let angle = value.trim().parse::<f32>().unwrap_or(0.0);
    let angles = Vec3::new(0.0, angle, 0.0);
    let _ = vm.write_edict_field_vec(ent, field, angles);
}

fn parse_vec3(value: &str) -> Option<Vec3> {
    let mut iter = value
        .split(|ch: char| ch == ' ' || ch == '\t')
        .filter(|part| !part.is_empty());
    let x = iter.next()?.parse::<f32>().ok()?;
    let y = iter.next()?.parse::<f32>().ok()?;
    let z = iter.next()?.parse::<f32>().ok()?;
    Some(Vec3::new(x, y, z))
}

fn push_unique(list: &mut Vec<String>, value: String) {
    if value.is_empty() {
        return;
    }
    if !list
        .iter()
        .any(|existing| existing.eq_ignore_ascii_case(&value))
    {
        list.push(value);
    }
}
