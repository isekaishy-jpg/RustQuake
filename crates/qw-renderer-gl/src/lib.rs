#[cfg(feature = "glow")]
use glow::HasContext;
#[cfg(feature = "glow")]
use qw_common::{AliasModel, MdlFrame, SpriteImage, Vec3};
use qw_renderer::{
    RenderDrawList, RenderEntity, RenderModel, RenderVertex, RenderView, RenderWorld, Renderer,
    RendererConfig, ResolvedEntity, UiLayer, build_draw_list,
};
#[cfg(any(feature = "glow", test))]
use qw_renderer::{
    RenderEntityKind, RenderModelFrame, RenderModelKind, RenderModelTexture, UiText,
};
#[cfg(feature = "glow")]
use std::ffi::c_void;

#[cfg(feature = "glow")]
pub struct GlDevice {
    gl: glow::Context,
}

#[cfg(feature = "glow")]
impl GlDevice {
    pub unsafe fn from_loader<F>(mut loader: F) -> Self
    where
        F: FnMut(&str) -> *const c_void,
    {
        let gl = glow::Context::from_loader_function(|name| loader(name));
        gl.enable(glow::DEPTH_TEST);
        gl.depth_func(glow::LEQUAL);
        gl.enable(glow::CULL_FACE);
        gl.cull_face(glow::BACK);
        Self { gl }
    }

    pub unsafe fn clear_color(&self, color: [f32; 4]) {
        self.gl.clear_color(color[0], color[1], color[2], color[3]);
    }

    pub unsafe fn clear(&self) {
        self.gl
            .clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);
    }

    pub unsafe fn viewport(&self, width: i32, height: i32) {
        self.gl.viewport(0, 0, width, height);
    }
}

#[cfg(feature = "glow")]
struct GlTexture {
    id: glow::Texture,
    width: u32,
    height: u32,
}

#[cfg(feature = "glow")]
impl GlTexture {
    unsafe fn from_rgba(device: &GlDevice, width: u32, height: u32, data: &[u8]) -> Self {
        let gl = &device.gl;
        let id = gl.create_texture().expect("gl texture");
        gl.bind_texture(glow::TEXTURE_2D, Some(id));
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MIN_FILTER,
            glow::LINEAR as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MAG_FILTER,
            glow::LINEAR as i32,
        );
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::REPEAT as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, glow::REPEAT as i32);
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::RGBA8 as i32,
            width as i32,
            height as i32,
            0,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            Some(data),
        );

        Self { id, width, height }
    }

    unsafe fn from_rgba_mips(device: &GlDevice, texture: &GpuTexture) -> Self {
        let gl = &device.gl;
        let id = gl.create_texture().expect("gl texture");
        gl.bind_texture(glow::TEXTURE_2D, Some(id));
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MIN_FILTER,
            glow::LINEAR_MIPMAP_LINEAR as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MAG_FILTER,
            glow::LINEAR as i32,
        );
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::REPEAT as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, glow::REPEAT as i32);

        for (level, mip) in texture.mips.iter().enumerate() {
            if mip.is_empty() {
                continue;
            }
            let level_width = (texture.width >> level).max(1) as i32;
            let level_height = (texture.height >> level).max(1) as i32;
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                level as i32,
                glow::RGBA8 as i32,
                level_width,
                level_height,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                Some(mip),
            );
        }
        gl.generate_mipmap(glow::TEXTURE_2D);

        Self {
            id,
            width: texture.width,
            height: texture.height,
        }
    }

    unsafe fn from_r8(device: &GlDevice, width: u32, height: u32, data: &[u8]) -> Self {
        let gl = &device.gl;
        let id = gl.create_texture().expect("gl lightmap");
        gl.bind_texture(glow::TEXTURE_2D, Some(id));
        gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MIN_FILTER,
            glow::LINEAR as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MAG_FILTER,
            glow::LINEAR as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_WRAP_S,
            glow::CLAMP_TO_EDGE as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_WRAP_T,
            glow::CLAMP_TO_EDGE as i32,
        );
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::R8 as i32,
            width as i32,
            height as i32,
            0,
            glow::RED,
            glow::UNSIGNED_BYTE,
            Some(data),
        );

        Self { id, width, height }
    }

    unsafe fn from_r8_nearest(device: &GlDevice, width: u32, height: u32, data: &[u8]) -> Self {
        let gl = &device.gl;
        let id = gl.create_texture().expect("gl texture");
        gl.bind_texture(glow::TEXTURE_2D, Some(id));
        gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MIN_FILTER,
            glow::NEAREST as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MAG_FILTER,
            glow::NEAREST as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_WRAP_S,
            glow::CLAMP_TO_EDGE as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_WRAP_T,
            glow::CLAMP_TO_EDGE as i32,
        );
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::R8 as i32,
            width as i32,
            height as i32,
            0,
            glow::RED,
            glow::UNSIGNED_BYTE,
            Some(data),
        );

        Self { id, width, height }
    }

    unsafe fn from_lightmap(device: &GlDevice, lightmap: &GpuLightmap) -> Self {
        let gl = &device.gl;
        let id = gl.create_texture().expect("gl lightmap");
        gl.bind_texture(glow::TEXTURE_2D, Some(id));
        gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MIN_FILTER,
            glow::LINEAR as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MAG_FILTER,
            glow::LINEAR as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_WRAP_S,
            glow::CLAMP_TO_EDGE as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_WRAP_T,
            glow::CLAMP_TO_EDGE as i32,
        );
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::R8 as i32,
            lightmap.width as i32,
            lightmap.height as i32,
            0,
            glow::RED,
            glow::UNSIGNED_BYTE,
            Some(&lightmap.samples),
        );

        Self {
            id,
            width: lightmap.width,
            height: lightmap.height,
        }
    }

    unsafe fn update_lightmap(&mut self, device: &GlDevice, lightmap: &GpuLightmap) {
        let gl = &device.gl;
        gl.bind_texture(glow::TEXTURE_2D, Some(self.id));
        gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
        if self.width != lightmap.width || self.height != lightmap.height {
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::R8 as i32,
                lightmap.width as i32,
                lightmap.height as i32,
                0,
                glow::RED,
                glow::UNSIGNED_BYTE,
                Some(&lightmap.samples),
            );
            self.width = lightmap.width;
            self.height = lightmap.height;
        } else {
            gl.tex_sub_image_2d(
                glow::TEXTURE_2D,
                0,
                0,
                0,
                lightmap.width as i32,
                lightmap.height as i32,
                glow::RED,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(&lightmap.samples),
            );
        }
    }
}

#[cfg(feature = "glow")]
struct GlProgram {
    program: glow::Program,
    view_proj: Option<glow::UniformLocation>,
    model: Option<glow::UniformLocation>,
    base_sampler: Option<glow::UniformLocation>,
    lightmap_sampler: Option<glow::UniformLocation>,
    debug_mode: Option<glow::UniformLocation>,
}

#[cfg(feature = "glow")]
impl GlProgram {
    unsafe fn new(device: &GlDevice) -> Self {
        let gl = &device.gl;
        let vertex_source = r#"#version 330 core
layout (location = 0) in vec3 a_pos;
layout (location = 1) in vec2 a_uv;
layout (location = 2) in vec2 a_light_uv;

uniform mat4 u_view_proj;
uniform mat4 u_model;

out vec2 v_uv;
out vec2 v_light_uv;

void main() {
    v_uv = a_uv;
    v_light_uv = a_light_uv;
    gl_Position = u_view_proj * u_model * vec4(a_pos, 1.0);
}
"#;
        let fragment_source = r#"#version 330 core
in vec2 v_uv;
in vec2 v_light_uv;

uniform sampler2D u_base_tex;
uniform sampler2D u_lightmap_tex;
uniform int u_debug_mode;

out vec4 frag_color;

void main() {
    vec4 base = texture(u_base_tex, v_uv);
    float light = texture(u_lightmap_tex, v_light_uv).r;
    if (u_debug_mode == 1) {
        frag_color = vec4(light, light, light, 1.0);
    } else {
        frag_color = vec4(base.rgb * light, base.a);
    }
}
"#;

        let vertex = compile_shader(gl, glow::VERTEX_SHADER, vertex_source);
        let fragment = compile_shader(gl, glow::FRAGMENT_SHADER, fragment_source);
        let program = gl.create_program().expect("gl program");
        gl.attach_shader(program, vertex);
        gl.attach_shader(program, fragment);
        gl.link_program(program);
        if !gl.get_program_link_status(program) {
            panic!(
                "gl program link failed: {}",
                gl.get_program_info_log(program)
            );
        }
        gl.detach_shader(program, vertex);
        gl.detach_shader(program, fragment);
        gl.delete_shader(vertex);
        gl.delete_shader(fragment);

        gl.use_program(Some(program));
        let view_proj = gl.get_uniform_location(program, "u_view_proj");
        let model = gl.get_uniform_location(program, "u_model");
        let base_sampler = gl.get_uniform_location(program, "u_base_tex");
        let lightmap_sampler = gl.get_uniform_location(program, "u_lightmap_tex");
        let debug_mode = gl.get_uniform_location(program, "u_debug_mode");
        if let Some(location) = base_sampler.as_ref() {
            gl.uniform_1_i32(Some(location), 0);
        }
        if let Some(location) = lightmap_sampler.as_ref() {
            gl.uniform_1_i32(Some(location), 1);
        }
        gl.use_program(None);

        Self {
            program,
            view_proj,
            model,
            base_sampler,
            lightmap_sampler,
            debug_mode,
        }
    }
}

#[cfg(feature = "glow")]
struct GlDynamicMesh {
    vao: glow::VertexArray,
    vbo: glow::Buffer,
    capacity: usize,
}

#[cfg(feature = "glow")]
impl GlDynamicMesh {
    unsafe fn new(device: &GlDevice) -> Self {
        let gl = &device.gl;
        let vao = gl.create_vertex_array().expect("gl vao");
        let vbo = gl.create_buffer().expect("gl vbo");
        gl.bind_vertex_array(Some(vao));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
        gl.buffer_data_size(glow::ARRAY_BUFFER, 0, glow::DYNAMIC_DRAW);
        let stride = (7 * std::mem::size_of::<f32>()) as i32;
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, stride, 0);
        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, stride, 12);
        gl.enable_vertex_attrib_array(2);
        gl.vertex_attrib_pointer_f32(2, 2, glow::FLOAT, false, stride, 20);
        gl.bind_vertex_array(None);
        Self {
            vao,
            vbo,
            capacity: 0,
        }
    }

    unsafe fn upload(&mut self, device: &GlDevice, vertices: &[f32]) {
        let gl = &device.gl;
        gl.bind_vertex_array(Some(self.vao));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));
        let byte_len = vertices.len() * std::mem::size_of::<f32>();
        let bytes = std::slice::from_raw_parts(vertices.as_ptr() as *const u8, byte_len);
        if byte_len > self.capacity {
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytes, glow::DYNAMIC_DRAW);
            self.capacity = byte_len;
        } else {
            gl.buffer_sub_data_u8_slice(glow::ARRAY_BUFFER, 0, bytes);
        }
    }
}

#[cfg(feature = "glow")]
struct GlModel {
    textures: Vec<GlTexture>,
}

#[cfg(feature = "glow")]
impl GlModel {
    unsafe fn from_render_model(device: &GlDevice, model: &RenderModel) -> Self {
        let textures = model
            .textures
            .iter()
            .map(|texture| {
                GlTexture::from_rgba(device, texture.width, texture.height, &texture.rgba)
            })
            .collect();
        Self { textures }
    }
}

#[cfg(feature = "glow")]
struct GlUiProgram {
    program: glow::Program,
    projection: Option<glow::UniformLocation>,
    color: Option<glow::UniformLocation>,
    font_sampler: Option<glow::UniformLocation>,
}

#[cfg(feature = "glow")]
impl GlUiProgram {
    unsafe fn new(device: &GlDevice) -> Self {
        let gl = &device.gl;
        let vertex_source = r#"#version 330 core
layout (location = 0) in vec2 a_pos;
layout (location = 1) in vec2 a_uv;

uniform mat4 u_projection;

out vec2 v_uv;

void main() {
    v_uv = a_uv;
    gl_Position = u_projection * vec4(a_pos.xy, 0.0, 1.0);
}
"#;
        let fragment_source = r#"#version 330 core
in vec2 v_uv;

uniform sampler2D u_font;
uniform vec4 u_color;

out vec4 frag_color;

void main() {
    float alpha = texture(u_font, v_uv).r;
    frag_color = vec4(u_color.rgb, u_color.a * alpha);
}
"#;

        let vertex = compile_shader(gl, glow::VERTEX_SHADER, vertex_source);
        let fragment = compile_shader(gl, glow::FRAGMENT_SHADER, fragment_source);
        let program = gl.create_program().expect("gl ui program");
        gl.attach_shader(program, vertex);
        gl.attach_shader(program, fragment);
        gl.link_program(program);
        if !gl.get_program_link_status(program) {
            panic!(
                "gl ui program link failed: {}",
                gl.get_program_info_log(program)
            );
        }
        gl.detach_shader(program, vertex);
        gl.detach_shader(program, fragment);
        gl.delete_shader(vertex);
        gl.delete_shader(fragment);

        gl.use_program(Some(program));
        let projection = gl.get_uniform_location(program, "u_projection");
        let color = gl.get_uniform_location(program, "u_color");
        let font_sampler = gl.get_uniform_location(program, "u_font");
        if let Some(location) = font_sampler.as_ref() {
            gl.uniform_1_i32(Some(location), 0);
        }
        gl.use_program(None);

        Self {
            program,
            projection,
            color,
            font_sampler,
        }
    }
}

#[cfg(feature = "glow")]
struct GlUiMesh {
    vao: glow::VertexArray,
    vbo: glow::Buffer,
    capacity: usize,
}

#[cfg(feature = "glow")]
impl GlUiMesh {
    unsafe fn new(device: &GlDevice) -> Self {
        let gl = &device.gl;
        let vao = gl.create_vertex_array().expect("gl ui vao");
        let vbo = gl.create_buffer().expect("gl ui vbo");
        gl.bind_vertex_array(Some(vao));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
        gl.buffer_data_size(glow::ARRAY_BUFFER, 0, glow::DYNAMIC_DRAW);
        let stride = (4 * std::mem::size_of::<f32>()) as i32;
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, stride, 0);
        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, stride, 8);
        gl.bind_vertex_array(None);
        Self {
            vao,
            vbo,
            capacity: 0,
        }
    }

    unsafe fn upload(&mut self, device: &GlDevice, vertices: &[f32]) {
        let gl = &device.gl;
        gl.bind_vertex_array(Some(self.vao));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));
        let byte_len = vertices.len() * std::mem::size_of::<f32>();
        let bytes = std::slice::from_raw_parts(vertices.as_ptr() as *const u8, byte_len);
        if byte_len > self.capacity {
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytes, glow::DYNAMIC_DRAW);
            self.capacity = byte_len;
        } else {
            gl.buffer_sub_data_u8_slice(glow::ARRAY_BUFFER, 0, bytes);
        }
    }
}

#[cfg(feature = "glow")]
struct GlState {
    program: GlProgram,
    fallback_base: GlTexture,
    fallback_lightmap: GlTexture,
    alias_mesh: GlDynamicMesh,
    ui_program: GlUiProgram,
    ui_font: GlTexture,
    ui_mesh: GlUiMesh,
}

#[cfg(feature = "glow")]
impl GlState {
    unsafe fn new(device: &GlDevice) -> Self {
        let program = GlProgram::new(device);
        let fallback_base = GlTexture::from_rgba(device, 1, 1, &[255, 255, 255, 255]);
        let fallback_lightmap = GlTexture::from_r8(device, 1, 1, &[255]);
        let alias_mesh = GlDynamicMesh::new(device);
        let ui_program = GlUiProgram::new(device);
        let font_data = build_font_texture();
        let ui_font =
            GlTexture::from_r8_nearest(device, FONT_ATLAS_WIDTH, FONT_ATLAS_HEIGHT, &font_data);
        let ui_mesh = GlUiMesh::new(device);
        Self {
            program,
            fallback_base,
            fallback_lightmap,
            alias_mesh,
            ui_program,
            ui_font,
            ui_mesh,
        }
    }
}

#[cfg(feature = "glow")]
unsafe fn compile_shader(gl: &glow::Context, shader_type: u32, source: &str) -> glow::Shader {
    let shader = gl.create_shader(shader_type).expect("gl shader");
    gl.shader_source(shader, source);
    gl.compile_shader(shader);
    if !gl.get_shader_compile_status(shader) {
        panic!(
            "gl shader compile failed: {}",
            gl.get_shader_info_log(shader)
        );
    }
    shader
}

#[cfg(feature = "glow")]
const FONT_CELL: u32 = 8;
#[cfg(feature = "glow")]
const FONT_ATLAS_COLS: u32 = 16;
#[cfg(feature = "glow")]
const FONT_ATLAS_ROWS: u32 = 8;
#[cfg(feature = "glow")]
const FONT_ATLAS_WIDTH: u32 = FONT_CELL * FONT_ATLAS_COLS;
#[cfg(feature = "glow")]
const FONT_ATLAS_HEIGHT: u32 = FONT_CELL * FONT_ATLAS_ROWS;

#[cfg(feature = "glow")]
const FONT8X8_BASIC: [u8; 1024] = [
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x3C, 0x3C, 0x18, 0x18, 0x00, 0x18,
    0x00, 0x36, 0x36, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x36, 0x36, 0x7F, 0x36, 0x7F, 0x36, 0x36,
    0x00, 0x0C, 0x3E, 0x03, 0x1E, 0x30, 0x1F, 0x0C, 0x00, 0x00, 0x63, 0x33, 0x18, 0x0C, 0x66, 0x63,
    0x00, 0x1C, 0x36, 0x1C, 0x6E, 0x3B, 0x33, 0x6E, 0x00, 0x06, 0x06, 0x03, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x18, 0x0C, 0x06, 0x06, 0x06, 0x0C, 0x18, 0x00, 0x06, 0x0C, 0x18, 0x18, 0x18, 0x0C, 0x06,
    0x00, 0x00, 0x66, 0x3C, 0xFF, 0x3C, 0x66, 0x00, 0x00, 0x00, 0x0C, 0x0C, 0x3F, 0x0C, 0x0C, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C, 0x06, 0x00, 0x00, 0x00, 0x3F, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C, 0x00, 0x60, 0x30, 0x18, 0x0C, 0x06, 0x03, 0x01,
    0x00, 0x3E, 0x63, 0x73, 0x7B, 0x6F, 0x67, 0x3E, 0x00, 0x0C, 0x0E, 0x0C, 0x0C, 0x0C, 0x0C, 0x3F,
    0x00, 0x1E, 0x33, 0x30, 0x1C, 0x06, 0x33, 0x3F, 0x00, 0x1E, 0x33, 0x30, 0x1C, 0x30, 0x33, 0x1E,
    0x00, 0x38, 0x3C, 0x36, 0x33, 0x7F, 0x30, 0x78, 0x00, 0x3F, 0x03, 0x1F, 0x30, 0x30, 0x33, 0x1E,
    0x00, 0x1C, 0x06, 0x03, 0x1F, 0x33, 0x33, 0x1E, 0x00, 0x3F, 0x33, 0x30, 0x18, 0x0C, 0x0C, 0x0C,
    0x00, 0x1E, 0x33, 0x33, 0x1E, 0x33, 0x33, 0x1E, 0x00, 0x1E, 0x33, 0x33, 0x3E, 0x30, 0x18, 0x0E,
    0x00, 0x00, 0x0C, 0x0C, 0x00, 0x00, 0x0C, 0x0C, 0x00, 0x00, 0x0C, 0x0C, 0x00, 0x00, 0x0C, 0x0C,
    0x06, 0x18, 0x0C, 0x06, 0x03, 0x06, 0x0C, 0x18, 0x00, 0x00, 0x00, 0x3F, 0x00, 0x00, 0x3F, 0x00,
    0x00, 0x06, 0x0C, 0x18, 0x30, 0x18, 0x0C, 0x06, 0x00, 0x1E, 0x33, 0x30, 0x18, 0x0C, 0x00, 0x0C,
    0x00, 0x3E, 0x63, 0x7B, 0x7B, 0x7B, 0x03, 0x1E, 0x00, 0x0C, 0x1E, 0x33, 0x33, 0x3F, 0x33, 0x33,
    0x00, 0x3F, 0x66, 0x66, 0x3E, 0x66, 0x66, 0x3F, 0x00, 0x3C, 0x66, 0x03, 0x03, 0x03, 0x66, 0x3C,
    0x00, 0x1F, 0x36, 0x66, 0x66, 0x66, 0x36, 0x1F, 0x00, 0x7F, 0x46, 0x16, 0x1E, 0x16, 0x46, 0x7F,
    0x00, 0x7F, 0x46, 0x16, 0x1E, 0x16, 0x06, 0x0F, 0x00, 0x3C, 0x66, 0x03, 0x03, 0x73, 0x66, 0x7C,
    0x00, 0x33, 0x33, 0x33, 0x3F, 0x33, 0x33, 0x33, 0x00, 0x1E, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x1E,
    0x00, 0x78, 0x30, 0x30, 0x30, 0x33, 0x33, 0x1E, 0x00, 0x67, 0x66, 0x36, 0x1E, 0x36, 0x66, 0x67,
    0x00, 0x0F, 0x06, 0x06, 0x06, 0x46, 0x66, 0x7F, 0x00, 0x63, 0x77, 0x7F, 0x7F, 0x6B, 0x63, 0x63,
    0x00, 0x63, 0x67, 0x6F, 0x7B, 0x73, 0x63, 0x63, 0x00, 0x1C, 0x36, 0x63, 0x63, 0x63, 0x36, 0x1C,
    0x00, 0x3F, 0x66, 0x66, 0x3E, 0x06, 0x06, 0x0F, 0x00, 0x1E, 0x33, 0x33, 0x33, 0x3B, 0x1E, 0x38,
    0x00, 0x3F, 0x66, 0x66, 0x3E, 0x36, 0x66, 0x67, 0x00, 0x1E, 0x33, 0x07, 0x0E, 0x38, 0x33, 0x1E,
    0x00, 0x3F, 0x2D, 0x0C, 0x0C, 0x0C, 0x0C, 0x1E, 0x00, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x3F,
    0x00, 0x33, 0x33, 0x33, 0x33, 0x33, 0x1E, 0x0C, 0x00, 0x63, 0x63, 0x63, 0x6B, 0x7F, 0x77, 0x63,
    0x00, 0x63, 0x63, 0x36, 0x1C, 0x1C, 0x36, 0x63, 0x00, 0x33, 0x33, 0x33, 0x1E, 0x0C, 0x0C, 0x1E,
    0x00, 0x7F, 0x63, 0x31, 0x18, 0x4C, 0x66, 0x7F, 0x00, 0x1E, 0x06, 0x06, 0x06, 0x06, 0x06, 0x1E,
    0x00, 0x03, 0x06, 0x0C, 0x18, 0x30, 0x60, 0x40, 0x00, 0x1E, 0x18, 0x18, 0x18, 0x18, 0x18, 0x1E,
    0x00, 0x08, 0x1C, 0x36, 0x63, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0xFF, 0x0C, 0x0C, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1E, 0x30, 0x3E, 0x33, 0x6E,
    0x00, 0x07, 0x06, 0x06, 0x3E, 0x66, 0x66, 0x3B, 0x00, 0x00, 0x00, 0x1E, 0x33, 0x03, 0x33, 0x1E,
    0x00, 0x38, 0x30, 0x30, 0x3E, 0x33, 0x33, 0x6E, 0x00, 0x00, 0x00, 0x1E, 0x33, 0x3F, 0x03, 0x1E,
    0x00, 0x1C, 0x36, 0x06, 0x0F, 0x06, 0x06, 0x0F, 0x00, 0x00, 0x00, 0x6E, 0x33, 0x33, 0x3E, 0x30,
    0x1F, 0x07, 0x06, 0x36, 0x6E, 0x66, 0x66, 0x67, 0x00, 0x0C, 0x00, 0x0E, 0x0C, 0x0C, 0x0C, 0x1E,
    0x00, 0x30, 0x00, 0x30, 0x30, 0x30, 0x33, 0x33, 0x1E, 0x07, 0x06, 0x66, 0x36, 0x1E, 0x36, 0x67,
    0x00, 0x0E, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x1E, 0x00, 0x00, 0x00, 0x33, 0x7F, 0x7F, 0x6B, 0x63,
    0x00, 0x00, 0x00, 0x1F, 0x33, 0x33, 0x33, 0x33, 0x00, 0x00, 0x00, 0x1E, 0x33, 0x33, 0x33, 0x1E,
    0x00, 0x00, 0x00, 0x3B, 0x66, 0x66, 0x3E, 0x06, 0x0F, 0x00, 0x00, 0x6E, 0x33, 0x33, 0x3E, 0x30,
    0x78, 0x00, 0x00, 0x3B, 0x6E, 0x66, 0x06, 0x0F, 0x00, 0x00, 0x3E, 0x03, 0x1E, 0x30, 0x1F, 0x00,
    0x08, 0x0C, 0x3E, 0x0C, 0x0C, 0x2C, 0x18, 0x00, 0x00, 0x00, 0x33, 0x33, 0x33, 0x33, 0x6E, 0x00,
    0x00, 0x00, 0x33, 0x33, 0x33, 0x1E, 0x0C, 0x00, 0x00, 0x00, 0x63, 0x6B, 0x7F, 0x7F, 0x36, 0x00,
    0x00, 0x00, 0x63, 0x36, 0x1C, 0x36, 0x63, 0x00, 0x00, 0x00, 0x33, 0x33, 0x33, 0x3E, 0x30, 0x1F,
    0x00, 0x00, 0x3F, 0x19, 0x0C, 0x26, 0x3F, 0x00, 0x38, 0x0C, 0x0C, 0x07, 0x0C, 0x0C, 0x38, 0x00,
    0x18, 0x18, 0x18, 0x00, 0x18, 0x18, 0x18, 0x00, 0x07, 0x0C, 0x0C, 0x38, 0x0C, 0x0C, 0x07, 0x00,
    0x6E, 0x3B, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

#[cfg(feature = "glow")]
fn build_font_texture() -> Vec<u8> {
    let mut data = vec![0u8; (FONT_ATLAS_WIDTH * FONT_ATLAS_HEIGHT) as usize];
    for glyph in 0..128u32 {
        let col = glyph % FONT_ATLAS_COLS;
        let row = glyph / FONT_ATLAS_COLS;
        for y in 0..FONT_CELL {
            let bits = FONT8X8_BASIC[(glyph * FONT_CELL + y) as usize];
            for x in 0..FONT_CELL {
                let on = (bits >> x) & 1;
                let px = col * FONT_CELL + x;
                let py = row * FONT_CELL + y;
                let idx = (py * FONT_ATLAS_WIDTH + px) as usize;
                data[idx] = if on == 1 { 255 } else { 0 };
            }
        }
    }
    data
}

#[cfg(feature = "glow")]
struct GlWorld {
    textures: Vec<GlTexture>,
    lightmaps: Vec<GlTexture>,
    mesh: GlMesh,
    draws: Vec<GlSurfaceDraw>,
}

#[cfg(feature = "glow")]
impl GlWorld {
    unsafe fn from_gpu_world(device: &GlDevice, world: &GpuWorld) -> Self {
        let textures = world
            .textures
            .iter()
            .map(|texture| GlTexture::from_rgba_mips(device, texture))
            .collect();
        let lightmaps = world
            .lightmaps
            .iter()
            .map(|lightmap| GlTexture::from_lightmap(device, lightmap))
            .collect();
        let (mesh, draws) = GlMesh::from_world(device, world);
        Self {
            textures,
            lightmaps,
            mesh,
            draws,
        }
    }

    unsafe fn update_lightmap(&mut self, device: &GlDevice, index: usize, lightmap: &GpuLightmap) {
        if let Some(texture) = self.lightmaps.get_mut(index) {
            texture.update_lightmap(device, lightmap);
        }
    }
}

#[cfg(feature = "glow")]
struct GlSurfaceDraw {
    index_offset: i32,
    index_count: i32,
    texture_index: Option<usize>,
    lightmap_index: Option<usize>,
}

#[cfg(feature = "glow")]
struct GlMesh {
    vao: glow::VertexArray,
    vbo: glow::Buffer,
    ibo: glow::Buffer,
}

#[cfg(feature = "glow")]
impl GlMesh {
    unsafe fn from_world(device: &GlDevice, world: &GpuWorld) -> (Self, Vec<GlSurfaceDraw>) {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut draws = Vec::new();
        let mut base_vertex = 0u32;

        for surface in &world.surfaces {
            let index_offset = indices.len() as i32;
            let index_count = surface.indices.len() as i32;
            indices.extend(surface.indices.iter().map(|idx| idx + base_vertex));
            draws.push(GlSurfaceDraw {
                index_offset,
                index_count,
                texture_index: surface.texture_index,
                lightmap_index: surface.lightmap_index,
            });

            for vertex in &surface.vertices {
                vertices.extend_from_slice(&[
                    vertex.position.x,
                    vertex.position.y,
                    vertex.position.z,
                    vertex.tex_coords[0],
                    vertex.tex_coords[1],
                    vertex.lightmap_coords[0],
                    vertex.lightmap_coords[1],
                ]);
            }
            base_vertex += surface.vertices.len() as u32;
        }

        let gl = &device.gl;
        let vao = gl.create_vertex_array().expect("gl vao");
        let vbo = gl.create_buffer().expect("gl vbo");
        let ibo = gl.create_buffer().expect("gl ibo");

        gl.bind_vertex_array(Some(vao));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
        let vertex_bytes = std::slice::from_raw_parts(
            vertices.as_ptr() as *const u8,
            vertices.len() * std::mem::size_of::<f32>(),
        );
        gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, vertex_bytes, glow::STATIC_DRAW);

        gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(ibo));
        let index_bytes = std::slice::from_raw_parts(
            indices.as_ptr() as *const u8,
            indices.len() * std::mem::size_of::<u32>(),
        );
        gl.buffer_data_u8_slice(glow::ELEMENT_ARRAY_BUFFER, index_bytes, glow::STATIC_DRAW);

        let stride = (7 * std::mem::size_of::<f32>()) as i32;
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, stride, 0);
        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, stride, 12);
        gl.enable_vertex_attrib_array(2);
        gl.vertex_attrib_pointer_f32(2, 2, glow::FLOAT, false, stride, 20);

        (GlMesh { vao, vbo, ibo }, draws)
    }
}

#[cfg(feature = "glow")]
#[derive(Debug, Clone, Copy, PartialEq)]
struct Mat4 {
    data: [f32; 16],
}

#[cfg(feature = "glow")]
impl Mat4 {
    fn identity() -> Self {
        Self {
            data: [
                1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
            ],
        }
    }

    fn mul(self, rhs: Mat4) -> Mat4 {
        let mut out = [0.0; 16];
        for col in 0..4 {
            for row in 0..4 {
                out[col * 4 + row] = self.data[0 * 4 + row] * rhs.data[col * 4 + 0]
                    + self.data[1 * 4 + row] * rhs.data[col * 4 + 1]
                    + self.data[2 * 4 + row] * rhs.data[col * 4 + 2]
                    + self.data[3 * 4 + row] * rhs.data[col * 4 + 3];
            }
        }
        Mat4 { data: out }
    }

    fn perspective(fov_y_deg: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
        let fov = fov_y_deg.max(1.0).min(179.0).to_radians();
        let f = 1.0 / (0.5 * fov).tan();
        let nf = 1.0 / (near - far);
        let mut out = [0.0; 16];
        out[0] = f / aspect;
        out[5] = f;
        out[10] = (far + near) * nf;
        out[11] = (2.0 * far * near) * nf;
        out[14] = -1.0;
        Mat4 { data: out }
    }

    fn orthographic(left: f32, right: f32, top: f32, bottom: f32, near: f32, far: f32) -> Mat4 {
        let rl = right - left;
        let tb = top - bottom;
        let fn_range = far - near;
        let mut out = [0.0; 16];
        out[0] = 2.0 / rl;
        out[5] = 2.0 / tb;
        out[10] = -2.0 / fn_range;
        out[12] = -(right + left) / rl;
        out[13] = -(top + bottom) / tb;
        out[14] = -(far + near) / fn_range;
        out[15] = 1.0;
        Mat4 { data: out }
    }

    fn from_basis(right: Vec3, up: Vec3, forward: Vec3, translation: Vec3) -> Mat4 {
        Mat4 {
            data: [
                right.x,
                right.y,
                right.z,
                0.0,
                up.x,
                up.y,
                up.z,
                0.0,
                forward.x,
                forward.y,
                forward.z,
                0.0,
                translation.x,
                translation.y,
                translation.z,
                1.0,
            ],
        }
    }

    fn view_from_angles(origin: Vec3, angles: Vec3) -> Mat4 {
        let (forward, right, up) = angle_vectors(angles);
        let neg_forward = forward.scale(-1.0);
        Mat4 {
            data: [
                right.x,
                right.y,
                right.z,
                -right.dot(origin),
                up.x,
                up.y,
                up.z,
                -up.dot(origin),
                neg_forward.x,
                neg_forward.y,
                neg_forward.z,
                forward.dot(origin),
                0.0,
                0.0,
                0.0,
                1.0,
            ],
        }
    }
}

#[cfg(feature = "glow")]
fn view_projection(view: RenderView, aspect: f32) -> Mat4 {
    let view_mat = Mat4::view_from_angles(view.origin, view.angles);
    let proj = Mat4::perspective(view.fov_y, aspect, 4.0, 8192.0);
    proj.mul(view_mat)
}

#[cfg(feature = "glow")]
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

#[cfg(feature = "glow")]
fn model_transform(
    origin: Vec3,
    angles: Vec3,
    model_origin: Vec3,
) -> (Mat4, Vec3, Vec3, Vec3, Vec3) {
    let (forward, right, up) = angle_vectors(angles);
    let rotated_origin = Vec3::new(
        right.x * model_origin.x + up.x * model_origin.y + forward.x * model_origin.z,
        right.y * model_origin.x + up.y * model_origin.y + forward.y * model_origin.z,
        right.z * model_origin.x + up.z * model_origin.y + forward.z * model_origin.z,
    );
    let translation = Vec3::new(
        origin.x - rotated_origin.x,
        origin.y - rotated_origin.y,
        origin.z - rotated_origin.z,
    );
    let matrix = Mat4::from_basis(right, up, forward, translation);
    (matrix, translation, forward, right, up)
}

#[cfg(feature = "glow")]
#[derive(Clone, Copy)]
struct FrustumPlane {
    normal: Vec3,
    d: f32,
}

#[cfg(feature = "glow")]
struct Frustum {
    planes: [FrustumPlane; 6],
}

#[cfg(feature = "glow")]
impl Frustum {
    fn from_view_proj(matrix: &Mat4) -> Self {
        let row = |r: usize| {
            [
                matrix.data[0 * 4 + r],
                matrix.data[1 * 4 + r],
                matrix.data[2 * 4 + r],
                matrix.data[3 * 4 + r],
            ]
        };
        let r0 = row(0);
        let r1 = row(1);
        let r2 = row(2);
        let r3 = row(3);

        let planes = [
            plane_from_rows(r3, r0, true),
            plane_from_rows(r3, r0, false),
            plane_from_rows(r3, r1, true),
            plane_from_rows(r3, r1, false),
            plane_from_rows(r3, r2, true),
            plane_from_rows(r3, r2, false),
        ];

        Self { planes }
    }

    fn contains_sphere(&self, center: Vec3, radius: f32) -> bool {
        for plane in &self.planes {
            let distance = plane.normal.dot(center) + plane.d;
            if distance < -radius {
                return false;
            }
        }
        true
    }
}

#[cfg(feature = "glow")]
fn plane_from_rows(row3: [f32; 4], row: [f32; 4], add: bool) -> FrustumPlane {
    let (a, b, c, d) = if add {
        (
            row3[0] + row[0],
            row3[1] + row[1],
            row3[2] + row[2],
            row3[3] + row[3],
        )
    } else {
        (
            row3[0] - row[0],
            row3[1] - row[1],
            row3[2] - row[2],
            row3[3] - row[3],
        )
    };
    let length = (a * a + b * b + c * c).sqrt();
    if length > 0.0 {
        FrustumPlane {
            normal: Vec3::new(a / length, b / length, c / length),
            d: d / length,
        }
    } else {
        FrustumPlane {
            normal: Vec3::default(),
            d: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GpuTexture {
    pub width: u32,
    pub height: u32,
    pub mips: [Vec<u8>; 4],
}

#[derive(Debug, Clone, PartialEq)]
pub struct GpuLightmap {
    pub width: u32,
    pub height: u32,
    pub samples: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GpuSurface {
    pub vertices: Vec<RenderVertex>,
    pub indices: Vec<u32>,
    pub texture_index: Option<usize>,
    pub lightmap_index: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GpuWorld {
    pub textures: Vec<GpuTexture>,
    pub lightmaps: Vec<GpuLightmap>,
    pub surfaces: Vec<GpuSurface>,
}

fn build_gpu_world(world: &RenderWorld) -> GpuWorld {
    let textures = world
        .textures
        .iter()
        .map(|texture| GpuTexture {
            width: texture.width,
            height: texture.height,
            mips: texture.mips.clone(),
        })
        .collect::<Vec<_>>();

    let mut lightmaps = Vec::new();
    let mut surfaces = Vec::with_capacity(world.surfaces.len());

    for surface in &world.surfaces {
        let lightmap_index = surface.lightmap.as_ref().map(|lightmap| {
            let mut samples = Vec::new();
            lightmap.write_combined_samples(&[], 0.0, &mut samples);
            let index = lightmaps.len();
            lightmaps.push(GpuLightmap {
                width: lightmap.width,
                height: lightmap.height,
                samples,
            });
            index
        });

        surfaces.push(GpuSurface {
            vertices: surface.vertices.clone(),
            indices: surface.indices.clone(),
            texture_index: surface.texture_index,
            lightmap_index,
        });
    }

    GpuWorld {
        textures,
        lightmaps,
        surfaces,
    }
}

#[derive(Debug, Clone)]
pub struct GlRenderer {
    config: RendererConfig,
    frame_index: u64,
    last_view: Option<RenderView>,
    last_world: Option<RenderWorld>,
    entities: Vec<RenderEntity>,
    models: Vec<Option<RenderModel>>,
    #[cfg(feature = "glow")]
    gl_device: Option<GlDevice>,
    #[cfg(feature = "glow")]
    gl_world: Option<GlWorld>,
    #[cfg(feature = "glow")]
    gl_state: Option<GlState>,
    #[cfg(feature = "glow")]
    gl_models: Vec<Option<GlModel>>,
    gpu_world: Option<GpuWorld>,
    ui: UiLayer,
    debug_wireframe: bool,
    debug_lightmap: bool,
}

impl GlRenderer {
    pub fn new(config: RendererConfig) -> Self {
        Self {
            config,
            frame_index: 0,
            last_view: None,
            last_world: None,
            entities: Vec::new(),
            models: Vec::new(),
            #[cfg(feature = "glow")]
            gl_device: None,
            #[cfg(feature = "glow")]
            gl_world: None,
            #[cfg(feature = "glow")]
            gl_state: None,
            #[cfg(feature = "glow")]
            gl_models: Vec::new(),
            gpu_world: None,
            ui: UiLayer::default(),
            debug_wireframe: false,
            debug_lightmap: false,
        }
    }

    pub fn frame_index(&self) -> u64 {
        self.frame_index
    }

    pub fn set_view(&mut self, view: RenderView) {
        self.last_view = Some(view);
    }

    pub fn view(&self) -> Option<RenderView> {
        self.last_view
    }

    pub fn set_world(&mut self, world: RenderWorld) {
        self.gpu_world = Some(build_gpu_world(&world));
        self.last_world = Some(world);
        #[cfg(feature = "glow")]
        if let (Some(device), Some(gpu_world)) = (&self.gl_device, &self.gpu_world) {
            unsafe {
                self.gl_world = Some(GlWorld::from_gpu_world(device, gpu_world));
            }
        }
    }

    pub fn world(&self) -> Option<&RenderWorld> {
        self.last_world.as_ref()
    }

    pub fn set_entities(&mut self, entities: Vec<RenderEntity>) {
        self.entities = entities;
    }

    pub fn set_models(&mut self, models: Vec<Option<RenderModel>>) {
        self.models = models;
        #[cfg(feature = "glow")]
        if self.gl_device.is_some() {
            self.rebuild_gl_models();
        }
    }

    pub fn models(&self) -> &[Option<RenderModel>] {
        &self.models
    }

    #[cfg(feature = "glow")]
    pub fn set_device(&mut self, device: GlDevice) {
        self.gl_device = Some(device);
        if let Some(device) = &self.gl_device {
            unsafe {
                device.viewport(self.config.width as i32, self.config.height as i32);
            }
        }
        if let Some(device) = &self.gl_device {
            unsafe {
                self.gl_state = Some(GlState::new(device));
            }
        }
        if let (Some(device), Some(gpu_world)) = (&self.gl_device, &self.gpu_world) {
            unsafe {
                self.gl_world = Some(GlWorld::from_gpu_world(device, gpu_world));
            }
        }
        self.rebuild_gl_models();
    }

    #[cfg(feature = "glow")]
    fn rebuild_gl_models(&mut self) {
        let Some(device) = self.gl_device.as_ref() else {
            self.gl_models.clear();
            return;
        };
        self.gl_models = self
            .models
            .iter()
            .map(|model| {
                model
                    .as_ref()
                    .map(|model| unsafe { GlModel::from_render_model(device, model) })
            })
            .collect();
    }

    pub fn resolved_entities(&self, time: f32) -> Vec<ResolvedEntity<'_>> {
        self.entities
            .iter()
            .map(|entity| {
                let model = self
                    .models
                    .get(entity.model_index)
                    .and_then(|entry| entry.as_ref());
                let frame = model.and_then(|model| model.frame_at_time(entity.frame, time));
                let texture_index =
                    model.and_then(|model| model.texture_index(entity.frame, entity.skin, time));
                ResolvedEntity {
                    entity,
                    model,
                    frame,
                    texture_index,
                }
            })
            .collect()
    }

    pub fn draw_list(&self) -> Option<RenderDrawList> {
        let world = self.last_world.as_ref()?;
        Some(build_draw_list(world, &self.entities))
    }

    pub fn set_ui(&mut self, ui: UiLayer) {
        self.ui = ui;
    }

    pub fn ui(&self) -> &UiLayer {
        &self.ui
    }

    pub fn set_wireframe(&mut self, enabled: bool) {
        self.debug_wireframe = enabled;
    }

    pub fn set_lightmap_debug(&mut self, enabled: bool) {
        self.debug_lightmap = enabled;
    }

    pub fn gpu_world(&self) -> Option<&GpuWorld> {
        self.gpu_world.as_ref()
    }

    pub fn update_lightmaps(&mut self, lightstyles: &[String], time: f32) {
        let (Some(world), Some(gpu_world)) = (&self.last_world, &mut self.gpu_world) else {
            return;
        };
        #[cfg(feature = "glow")]
        let gl_device = self.gl_device.as_ref();
        #[cfg(feature = "glow")]
        let mut gl_world = self.gl_world.as_mut();

        for (surface_index, surface) in world.surfaces.iter().enumerate() {
            let Some(lightmap) = surface.lightmap.as_ref() else {
                continue;
            };
            let Some(lightmap_index) = gpu_world
                .surfaces
                .get(surface_index)
                .and_then(|gpu_surface| gpu_surface.lightmap_index)
            else {
                continue;
            };
            let Some(gpu_lightmap) = gpu_world.lightmaps.get_mut(lightmap_index) else {
                continue;
            };
            gpu_lightmap.width = lightmap.width;
            gpu_lightmap.height = lightmap.height;
            lightmap.write_combined_samples(lightstyles, time, &mut gpu_lightmap.samples);
            #[cfg(feature = "glow")]
            if let (Some(device), Some(gl_world)) = (gl_device, gl_world.as_deref_mut()) {
                unsafe {
                    gl_world.update_lightmap(device, lightmap_index, gpu_lightmap);
                }
            }
        }
    }

    #[cfg(feature = "glow")]
    fn draw_world(&mut self) {
        let Some(draw_list) = self.draw_list() else {
            return;
        };
        let (Some(device), Some(gl_state), Some(gl_world), Some(view), Some(world)) = (
            self.gl_device.as_ref(),
            self.gl_state.as_mut(),
            self.gl_world.as_ref(),
            self.last_view,
            self.last_world.as_ref(),
        ) else {
            return;
        };

        let aspect = self.config.width as f32 / self.config.height as f32;
        let view_proj = view_projection(view, aspect);
        let frustum = Frustum::from_view_proj(&view_proj);
        let model_identity = Mat4::identity();
        let gl = &device.gl;
        let time = self.frame_index as f32 * (1.0 / 60.0);
        unsafe {
            gl.use_program(Some(gl_state.program.program));
            if let Some(location) = gl_state.program.view_proj.as_ref() {
                gl.uniform_matrix_4_f32_slice(Some(location), false, &view_proj.data);
            }
            if let Some(location) = gl_state.program.model.as_ref() {
                gl.uniform_matrix_4_f32_slice(Some(location), false, &model_identity.data);
            }
            let debug_mode = if self.debug_lightmap { 1 } else { 0 };
            if let Some(location) = gl_state.program.debug_mode.as_ref() {
                gl.uniform_1_i32(Some(location), debug_mode);
            }
            if self.debug_wireframe {
                gl.polygon_mode(glow::FRONT_AND_BACK, glow::LINE);
            } else {
                gl.polygon_mode(glow::FRONT_AND_BACK, glow::FILL);
            }

            gl.bind_vertex_array(Some(gl_world.mesh.vao));

            let mut draw_surface = |surface_index: usize| {
                if let Some(surface) = world.surfaces.get(surface_index) {
                    if !frustum.contains_sphere(surface.bounds.center, surface.bounds.radius) {
                        return;
                    }
                }
                let Some(draw) = gl_world.draws.get(surface_index) else {
                    return;
                };
                if draw.index_count <= 0 {
                    return;
                }
                let base = draw
                    .texture_index
                    .and_then(|index| gl_world.textures.get(index))
                    .unwrap_or(&gl_state.fallback_base);
                let lightmap = draw
                    .lightmap_index
                    .and_then(|index| gl_world.lightmaps.get(index))
                    .unwrap_or(&gl_state.fallback_lightmap);
                gl.active_texture(glow::TEXTURE0);
                gl.bind_texture(glow::TEXTURE_2D, Some(base.id));
                gl.active_texture(glow::TEXTURE1);
                gl.bind_texture(glow::TEXTURE_2D, Some(lightmap.id));
                let offset_bytes = draw.index_offset * std::mem::size_of::<u32>() as i32;
                gl.draw_elements(
                    glow::TRIANGLES,
                    draw.index_count,
                    glow::UNSIGNED_INT,
                    offset_bytes,
                );
            };

            gl.disable(glow::BLEND);
            gl.depth_mask(true);
            for &surface_index in &draw_list.opaque_surfaces {
                draw_surface(surface_index);
            }
            self.draw_brush_entities(
                device,
                gl_state,
                &self.entities,
                world,
                gl_world,
                &frustum,
                false,
            );
            if let Some(location) = gl_state.program.model.as_ref() {
                gl.uniform_matrix_4_f32_slice(Some(location), false, &model_identity.data);
            }
            if let Some(location) = gl_state.program.debug_mode.as_ref() {
                gl.uniform_1_i32(Some(location), 0);
            }
            self.draw_alias_entities(device, gl_state, &draw_list.opaque_entities, &frustum, time);
            self.draw_sprite_entities(
                device,
                gl_state,
                &draw_list.opaque_entities,
                view,
                &frustum,
                time,
            );

            gl.enable(glow::BLEND);
            gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
            gl.depth_mask(false);
            if let Some(location) = gl_state.program.debug_mode.as_ref() {
                gl.uniform_1_i32(Some(location), debug_mode);
            }
            gl.bind_vertex_array(Some(gl_world.mesh.vao));
            for &surface_index in &draw_list.transparent_surfaces {
                draw_surface(surface_index);
            }
            self.draw_brush_entities(
                device,
                gl_state,
                &self.entities,
                world,
                gl_world,
                &frustum,
                true,
            );
            if let Some(location) = gl_state.program.model.as_ref() {
                gl.uniform_matrix_4_f32_slice(Some(location), false, &model_identity.data);
            }
            if let Some(location) = gl_state.program.debug_mode.as_ref() {
                gl.uniform_1_i32(Some(location), 0);
            }
            self.draw_alias_entities(
                device,
                gl_state,
                &draw_list.transparent_entities,
                &frustum,
                time,
            );
            self.draw_sprite_entities(
                device,
                gl_state,
                &draw_list.transparent_entities,
                view,
                &frustum,
                time,
            );
            gl.depth_mask(true);
            if self.debug_wireframe {
                gl.polygon_mode(glow::FRONT_AND_BACK, glow::FILL);
            }
            self.draw_ui(device, gl_state);

            gl.bind_vertex_array(None);
            gl.use_program(None);
        }
    }

    #[cfg(feature = "glow")]
    fn draw_brush_entities(
        &self,
        device: &GlDevice,
        gl_state: &mut GlState,
        entities: &[RenderEntity],
        world: &RenderWorld,
        gl_world: &GlWorld,
        frustum: &Frustum,
        transparent: bool,
    ) {
        let gl = &device.gl;
        for entity in entities {
            if entity.kind != RenderEntityKind::Brush {
                continue;
            }
            if entity.model_index == 0 {
                continue;
            }
            let brush_index = entity.model_index.saturating_sub(1);
            if brush_index == 0 {
                continue;
            }
            let Some(brush_model) = world.brush_models.get(brush_index) else {
                continue;
            };
            if brush_model.surfaces.is_empty() {
                continue;
            }
            let (model_matrix, translation, forward, right, up) =
                model_transform(entity.origin, entity.angles, brush_model.origin);
            let center = brush_model.bounds.center;
            let world_center = Vec3::new(
                translation.x + right.x * center.x + up.x * center.y + forward.x * center.z,
                translation.y + right.y * center.x + up.y * center.y + forward.y * center.z,
                translation.z + right.z * center.x + up.z * center.y + forward.z * center.z,
            );
            if !frustum.contains_sphere(world_center, brush_model.bounds.radius) {
                continue;
            }
            if let Some(location) = gl_state.program.model.as_ref() {
                unsafe {
                    gl.uniform_matrix_4_f32_slice(Some(location), false, &model_matrix.data);
                }
            }
            let entity_transparent = entity.alpha < 1.0;
            for &surface_index in &brush_model.surfaces {
                let Some(surface) = world.surfaces.get(surface_index) else {
                    continue;
                };
                let surface_transparent = surface
                    .texture_name
                    .as_deref()
                    .is_some_and(|name| name.starts_with('{'));
                if entity_transparent {
                    if !transparent {
                        continue;
                    }
                } else if surface_transparent != transparent {
                    continue;
                }
                let Some(draw) = gl_world.draws.get(surface_index) else {
                    continue;
                };
                if draw.index_count <= 0 {
                    continue;
                }
                let base = draw
                    .texture_index
                    .and_then(|index| gl_world.textures.get(index))
                    .unwrap_or(&gl_state.fallback_base);
                let lightmap = draw
                    .lightmap_index
                    .and_then(|index| gl_world.lightmaps.get(index))
                    .unwrap_or(&gl_state.fallback_lightmap);
                unsafe {
                    gl.active_texture(glow::TEXTURE0);
                    gl.bind_texture(glow::TEXTURE_2D, Some(base.id));
                    gl.active_texture(glow::TEXTURE1);
                    gl.bind_texture(glow::TEXTURE_2D, Some(lightmap.id));
                    let offset_bytes = draw.index_offset * std::mem::size_of::<u32>() as i32;
                    gl.draw_elements(
                        glow::TRIANGLES,
                        draw.index_count,
                        glow::UNSIGNED_INT,
                        offset_bytes,
                    );
                }
            }
        }
    }

    #[cfg(feature = "glow")]
    fn draw_alias_entities(
        &self,
        device: &GlDevice,
        gl_state: &mut GlState,
        entities: &[RenderEntity],
        frustum: &Frustum,
        time: f32,
    ) {
        let gl = &device.gl;
        unsafe {
            gl.disable(glow::CULL_FACE);
        }
        for entity in entities {
            if entity.kind != RenderEntityKind::Alias {
                continue;
            }
            let Some(model) = self
                .models
                .get(entity.model_index)
                .and_then(|entry| entry.as_ref())
            else {
                continue;
            };
            let Some(frame) = model.frame_at_time(entity.frame, time) else {
                continue;
            };
            let RenderModelFrame::Alias(frame) = frame else {
                continue;
            };
            let RenderModelKind::Alias(alias_model) = &model.kind else {
                continue;
            };
            if !frustum.contains_sphere(entity.origin, alias_model.header.bounding_radius) {
                continue;
            }
            let texture_index = model.texture_index(entity.frame, entity.skin, time);
            let gl_model = self
                .gl_models
                .get(entity.model_index)
                .and_then(|entry| entry.as_ref());
            let base = texture_index
                .and_then(|index| gl_model.and_then(|model| model.textures.get(index)))
                .unwrap_or(&gl_state.fallback_base);

            let vertices = build_alias_vertices(alias_model, frame, entity);
            if vertices.is_empty() {
                continue;
            }
            unsafe {
                gl_state.alias_mesh.upload(device, &vertices);
                gl.bind_vertex_array(Some(gl_state.alias_mesh.vao));
                gl.active_texture(glow::TEXTURE0);
                gl.bind_texture(glow::TEXTURE_2D, Some(base.id));
                gl.active_texture(glow::TEXTURE1);
                gl.bind_texture(glow::TEXTURE_2D, Some(gl_state.fallback_lightmap.id));
                gl.draw_arrays(glow::TRIANGLES, 0, (vertices.len() / 7) as i32);
            }
        }
        unsafe {
            gl.enable(glow::CULL_FACE);
        }
    }

    #[cfg(feature = "glow")]
    fn draw_sprite_entities(
        &self,
        device: &GlDevice,
        gl_state: &mut GlState,
        entities: &[RenderEntity],
        view: RenderView,
        frustum: &Frustum,
        time: f32,
    ) {
        let gl = &device.gl;
        let (_, view_right, view_up) = angle_vectors(view.angles);
        unsafe {
            gl.disable(glow::CULL_FACE);
        }
        for entity in entities {
            if entity.kind != RenderEntityKind::Sprite {
                continue;
            }
            let Some(model) = self
                .models
                .get(entity.model_index)
                .and_then(|entry| entry.as_ref())
            else {
                continue;
            };
            let Some(frame) = model.frame_at_time(entity.frame, time) else {
                continue;
            };
            let RenderModelFrame::Sprite(image) = frame else {
                continue;
            };
            let radius = 0.5 * image.width.max(image.height) as f32;
            if !frustum.contains_sphere(entity.origin, radius) {
                continue;
            }
            let texture_index = model.texture_index(entity.frame, entity.skin, time);
            let gl_model = self
                .gl_models
                .get(entity.model_index)
                .and_then(|entry| entry.as_ref());
            let base = texture_index
                .and_then(|index| gl_model.and_then(|model| model.textures.get(index)))
                .unwrap_or(&gl_state.fallback_base);

            let vertices = build_sprite_vertices(entity, image, view_right, view_up);
            if vertices.is_empty() {
                continue;
            }
            unsafe {
                gl_state.alias_mesh.upload(device, &vertices);
                gl.bind_vertex_array(Some(gl_state.alias_mesh.vao));
                gl.active_texture(glow::TEXTURE0);
                gl.bind_texture(glow::TEXTURE_2D, Some(base.id));
                gl.active_texture(glow::TEXTURE1);
                gl.bind_texture(glow::TEXTURE_2D, Some(gl_state.fallback_lightmap.id));
                gl.draw_arrays(glow::TRIANGLES, 0, (vertices.len() / 7) as i32);
            }
        }
        unsafe {
            gl.enable(glow::CULL_FACE);
        }
    }

    #[cfg(feature = "glow")]
    fn draw_ui(&self, device: &GlDevice, gl_state: &mut GlState) {
        if self.ui.texts.is_empty() {
            return;
        }
        let gl = &device.gl;
        let projection = Mat4::orthographic(
            0.0,
            self.config.width as f32,
            0.0,
            self.config.height as f32,
            -1.0,
            1.0,
        );
        unsafe {
            gl.use_program(Some(gl_state.ui_program.program));
            if let Some(location) = gl_state.ui_program.projection.as_ref() {
                gl.uniform_matrix_4_f32_slice(Some(location), false, &projection.data);
            }
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(gl_state.ui_font.id));
            gl.enable(glow::BLEND);
            gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
            gl.disable(glow::DEPTH_TEST);
            gl.bind_vertex_array(Some(gl_state.ui_mesh.vao));
        }

        for text in &self.ui.texts {
            let vertices = build_ui_vertices(text);
            if vertices.is_empty() {
                continue;
            }
            let color = [
                text.color[0] as f32 / 255.0,
                text.color[1] as f32 / 255.0,
                text.color[2] as f32 / 255.0,
                text.color[3] as f32 / 255.0,
            ];
            unsafe {
                if let Some(location) = gl_state.ui_program.color.as_ref() {
                    gl.uniform_4_f32(Some(location), color[0], color[1], color[2], color[3]);
                }
                gl_state.ui_mesh.upload(device, &vertices);
                gl.draw_arrays(glow::TRIANGLES, 0, (vertices.len() / 4) as i32);
            }
        }

        unsafe {
            gl.bind_vertex_array(None);
            gl.enable(glow::DEPTH_TEST);
            gl.use_program(None);
        }
    }
}

impl Renderer for GlRenderer {
    fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
        #[cfg(feature = "glow")]
        if let Some(device) = &self.gl_device {
            unsafe {
                device.viewport(self.config.width as i32, self.config.height as i32);
            }
        }
    }

    fn begin_frame(&mut self) {
        self.frame_index = self.frame_index.wrapping_add(1);
        #[cfg(feature = "glow")]
        if let Some(device) = &self.gl_device {
            unsafe {
                device.clear_color([0.05, 0.05, 0.05, 1.0]);
                device.clear();
            }
        }
    }

    fn end_frame(&mut self) {
        #[cfg(feature = "glow")]
        self.draw_world();
    }

    fn config(&self) -> RendererConfig {
        self.config
    }
}

#[cfg(feature = "glow")]
fn build_alias_vertices(model: &AliasModel, frame: &MdlFrame, entity: &RenderEntity) -> Vec<f32> {
    let width = model.header.skin_width as f32;
    let height = model.header.skin_height as f32;
    if width <= 0.0 || height <= 0.0 {
        return Vec::new();
    }
    let mut vertices = Vec::with_capacity(model.triangles.len() * 3 * 7);
    for triangle in &model.triangles {
        for &index in &triangle.indices {
            let idx = index as usize;
            let Some(vertex) = frame.vertices.get(idx) else {
                continue;
            };
            let Some(tex_coord) = model.tex_coords.get(idx) else {
                continue;
            };
            let mut s = tex_coord.s as f32;
            if !triangle.faces_front && tex_coord.on_seam {
                s += width * 0.5;
            }
            let u = s / width;
            let v = tex_coord.t as f32 / height;
            let pos = transform_alias_vertex(vertex.position, entity.origin, entity.angles);
            vertices.extend_from_slice(&[pos.x, pos.y, pos.z, u, v, 0.0, 0.0]);
        }
    }
    vertices
}

#[cfg(feature = "glow")]
fn transform_alias_vertex(position: Vec3, origin: Vec3, angles: Vec3) -> Vec3 {
    let (forward, right, up) = angle_vectors(angles);
    Vec3::new(
        origin.x + forward.x * position.x + right.x * position.y + up.x * position.z,
        origin.y + forward.y * position.x + right.y * position.y + up.y * position.z,
        origin.z + forward.z * position.x + right.z * position.y + up.z * position.z,
    )
}

#[cfg(feature = "glow")]
fn build_sprite_vertices(
    entity: &RenderEntity,
    image: &SpriteImage,
    view_right: Vec3,
    view_up: Vec3,
) -> Vec<f32> {
    let width = image.width as f32;
    let height = image.height as f32;
    if width <= 0.0 || height <= 0.0 {
        return Vec::new();
    }
    let left = -(image.origin.0 as f32);
    let top = -(image.origin.1 as f32);
    let right = left + width;
    let bottom = top + height;

    let make_point = |x: f32, y: f32| {
        Vec3::new(
            entity.origin.x + view_right.x * x + view_up.x * y,
            entity.origin.y + view_right.y * x + view_up.y * y,
            entity.origin.z + view_right.z * x + view_up.z * y,
        )
    };

    let p0 = make_point(left, top);
    let p1 = make_point(right, top);
    let p2 = make_point(right, bottom);
    let p3 = make_point(left, bottom);

    let mut vertices = Vec::with_capacity(6 * 7);
    vertices.extend_from_slice(&[p0.x, p0.y, p0.z, 0.0, 0.0, 0.0, 0.0]);
    vertices.extend_from_slice(&[p1.x, p1.y, p1.z, 1.0, 0.0, 0.0, 0.0]);
    vertices.extend_from_slice(&[p2.x, p2.y, p2.z, 1.0, 1.0, 0.0, 0.0]);
    vertices.extend_from_slice(&[p0.x, p0.y, p0.z, 0.0, 0.0, 0.0, 0.0]);
    vertices.extend_from_slice(&[p2.x, p2.y, p2.z, 1.0, 1.0, 0.0, 0.0]);
    vertices.extend_from_slice(&[p3.x, p3.y, p3.z, 0.0, 1.0, 0.0, 0.0]);
    vertices
}

#[cfg(feature = "glow")]
fn build_ui_vertices(text: &UiText) -> Vec<f32> {
    let cell = FONT_CELL as f32;
    let atlas_w = FONT_ATLAS_WIDTH as f32;
    let atlas_h = FONT_ATLAS_HEIGHT as f32;
    let mut vertices = Vec::with_capacity(text.text.len() * 6 * 4);
    let mut cursor_x = text.x as f32;
    let mut cursor_y = text.y as f32;

    for ch in text.text.bytes() {
        if ch == b'\n' {
            cursor_x = text.x as f32;
            cursor_y += cell;
            continue;
        }
        let glyph = if ch < 128 { ch } else { b'?' } as u32;
        let col = glyph % FONT_ATLAS_COLS;
        let row = glyph / FONT_ATLAS_COLS;
        let u0 = (col * FONT_CELL) as f32 / atlas_w;
        let v0 = (row * FONT_CELL) as f32 / atlas_h;
        let u1 = ((col + 1) * FONT_CELL) as f32 / atlas_w;
        let v1 = ((row + 1) * FONT_CELL) as f32 / atlas_h;

        let x0 = cursor_x;
        let y0 = cursor_y;
        let x1 = cursor_x + cell;
        let y1 = cursor_y + cell;

        vertices.extend_from_slice(&[x0, y0, u0, v0]);
        vertices.extend_from_slice(&[x1, y0, u1, v0]);
        vertices.extend_from_slice(&[x1, y1, u1, v1]);
        vertices.extend_from_slice(&[x0, y0, u0, v0]);
        vertices.extend_from_slice(&[x1, y1, u1, v1]);
        vertices.extend_from_slice(&[x0, y1, u0, v1]);

        cursor_x += cell;
    }

    vertices
}

#[cfg(test)]
mod tests {
    use super::*;
    use qw_common::{BspRender, Sprite, SpriteFrame, SpriteHeader, SpriteImage, Vec3};

    #[test]
    fn resizes_to_nonzero_dimensions() {
        let mut renderer = GlRenderer::new(RendererConfig::default());
        renderer.resize(0, 0);
        let cfg = renderer.config();
        assert_eq!(cfg.width, 1);
        assert_eq!(cfg.height, 1);
    }

    #[test]
    fn increments_frame_index() {
        let mut renderer = GlRenderer::new(RendererConfig::default());
        assert_eq!(renderer.frame_index(), 0);
        renderer.begin_frame();
        assert_eq!(renderer.frame_index(), 1);
    }

    #[test]
    fn stores_view_state() {
        let mut renderer = GlRenderer::new(RendererConfig::default());
        let view = RenderView {
            origin: Vec3::new(1.0, 2.0, 3.0),
            angles: Vec3::new(10.0, 20.0, 30.0),
            fov_y: 90.0,
        };
        renderer.set_view(view);
        assert_eq!(renderer.view(), Some(view));
    }

    #[test]
    fn stores_world_state() {
        let mut renderer = GlRenderer::new(RendererConfig::default());
        let world = RenderWorld::from_bsp(
            "maps/start.bsp",
            BspRender {
                vertices: Vec::new(),
                edges: Vec::new(),
                surf_edges: Vec::new(),
                texinfo: Vec::new(),
                faces: Vec::new(),
                textures: Vec::new(),
                lighting: Vec::new(),
                models: Vec::new(),
            },
        );
        renderer.set_world(world.clone());
        assert_eq!(renderer.world(), Some(&world));
    }

    #[test]
    fn stores_model_state() {
        let mut renderer = GlRenderer::new(RendererConfig::default());
        let sprite = Sprite {
            header: SpriteHeader {
                sprite_type: 0,
                bounding_radius: 0.0,
                width: 1,
                height: 1,
                num_frames: 0,
                beam_length: 0.0,
                sync_type: 0,
            },
            frames: Vec::new(),
        };
        let models = vec![Some(RenderModel {
            kind: RenderModelKind::Sprite(sprite),
            textures: vec![RenderModelTexture {
                width: 1,
                height: 1,
                rgba: vec![0, 0, 0, 255],
            }],
        })];
        renderer.set_models(models.clone());
        assert_eq!(renderer.models(), &models);
    }

    #[test]
    fn resolves_entity_frames_from_models() {
        let sprite = Sprite {
            header: SpriteHeader {
                sprite_type: 0,
                bounding_radius: 0.0,
                width: 1,
                height: 1,
                num_frames: 1,
                beam_length: 0.0,
                sync_type: 0,
            },
            frames: vec![SpriteFrame::Single(SpriteImage {
                width: 1,
                height: 1,
                origin: (0, 0),
                pixels: vec![9],
            })],
        };
        let model = RenderModel {
            kind: RenderModelKind::Sprite(sprite),
            textures: vec![RenderModelTexture {
                width: 1,
                height: 1,
                rgba: vec![9, 9, 9, 255],
            }],
        };

        let mut renderer = GlRenderer::new(RendererConfig::default());
        renderer.set_models(vec![Some(model)]);
        renderer.set_entities(vec![RenderEntity {
            kind: RenderEntityKind::Sprite,
            model_index: 0,
            origin: Vec3::default(),
            angles: Vec3::default(),
            frame: 0,
            skin: 0,
            alpha: 1.0,
        }]);

        let resolved = renderer.resolved_entities(0.0);
        assert_eq!(resolved.len(), 1);
        let frame = resolved[0].frame.unwrap();
        match frame {
            RenderModelFrame::Sprite(image) => {
                assert_eq!(image.pixels[0], 9);
            }
            _ => panic!("expected sprite frame"),
        }
        assert_eq!(resolved[0].texture_index, Some(0));
    }

    #[test]
    fn stores_ui_state() {
        let mut renderer = GlRenderer::new(RendererConfig::default());
        let ui = UiLayer {
            texts: vec![UiText {
                text: "Loading...".to_string(),
                x: 10,
                y: 20,
                color: [255, 255, 255, 255],
            }],
        };
        renderer.set_ui(ui.clone());
        assert_eq!(renderer.ui(), &ui);
    }

    #[test]
    fn builds_gpu_world_lightmaps() {
        let mut lighting = Vec::new();
        lighting.extend_from_slice(&[5u8; 9]);

        let bsp = BspRender {
            vertices: vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(32.0, 0.0, 0.0),
                Vec3::new(32.0, 32.0, 0.0),
                Vec3::new(0.0, 32.0, 0.0),
            ],
            edges: vec![[0, 1], [1, 2], [2, 3], [3, 0]],
            surf_edges: vec![0, 1, 2, 3],
            texinfo: vec![qw_common::TexInfo {
                s_vec: Vec3::new(1.0, 0.0, 0.0),
                s_offset: 0.0,
                t_vec: Vec3::new(0.0, 1.0, 0.0),
                t_offset: 0.0,
                texture_id: 0,
                flags: 0,
            }],
            faces: vec![qw_common::Face {
                plane_num: 0,
                side: 0,
                first_edge: 0,
                num_edges: 4,
                texinfo: 0,
                styles: [0, 255, 255, 255],
                light_ofs: 0,
            }],
            textures: vec![qw_common::BspTexture {
                name: "wall".to_string(),
                width: 32,
                height: 32,
                offsets: [0; 4],
                mip_data: None,
            }],
            lighting,
            models: Vec::new(),
        };

        let world = RenderWorld::from_bsp("maps/test.bsp", bsp);
        let mut renderer = GlRenderer::new(RendererConfig::default());
        renderer.set_world(world);
        let gpu_world = renderer.gpu_world().unwrap();
        assert_eq!(gpu_world.lightmaps.len(), 1);
        assert_eq!(gpu_world.surfaces[0].lightmap_index, Some(0));
        assert_eq!(gpu_world.lightmaps[0].samples.len(), 9);
    }

    #[test]
    fn updates_gpu_lightmaps_from_styles() {
        let mut lighting = Vec::new();
        lighting.extend_from_slice(&[100u8; 9]);

        let bsp = BspRender {
            vertices: vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(32.0, 0.0, 0.0),
                Vec3::new(32.0, 32.0, 0.0),
                Vec3::new(0.0, 32.0, 0.0),
            ],
            edges: vec![[0, 1], [1, 2], [2, 3], [3, 0]],
            surf_edges: vec![0, 1, 2, 3],
            texinfo: vec![qw_common::TexInfo {
                s_vec: Vec3::new(1.0, 0.0, 0.0),
                s_offset: 0.0,
                t_vec: Vec3::new(0.0, 1.0, 0.0),
                t_offset: 0.0,
                texture_id: 0,
                flags: 0,
            }],
            faces: vec![qw_common::Face {
                plane_num: 0,
                side: 0,
                first_edge: 0,
                num_edges: 4,
                texinfo: 0,
                styles: [0, 255, 255, 255],
                light_ofs: 0,
            }],
            textures: vec![qw_common::BspTexture {
                name: "wall".to_string(),
                width: 32,
                height: 32,
                offsets: [0; 4],
                mip_data: None,
            }],
            lighting,
            models: Vec::new(),
        };

        let world = RenderWorld::from_bsp("maps/test.bsp", bsp);
        let mut renderer = GlRenderer::new(RendererConfig::default());
        renderer.set_world(world);

        let mut styles = vec![String::new(); 1];
        styles[0] = "b".to_string();
        renderer.update_lightmaps(&styles, 0.0);

        let gpu_world = renderer.gpu_world().unwrap();
        assert_eq!(gpu_world.lightmaps[0].samples[0], 4);
    }
}
