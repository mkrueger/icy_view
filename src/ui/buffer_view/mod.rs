use eframe::epaint::Vec2;
use glow::NativeTexture;
use icy_engine::{Buffer, Caret};

pub mod render;
pub use render::*;

pub mod sixel;
pub use sixel::*;

pub mod selection;
pub use selection::*;

// use super::main_window::{Options, PostProcessing, Scaling};

pub struct Blink {
    is_on: bool,
    last_blink: u128,
    blink_rate: u128,
}

impl Blink {
    pub fn new(blink_rate: u128) -> Self {
        Self {
            is_on: false,
            last_blink: 0,
            blink_rate,
        }
    }

    pub fn is_on(&self) -> bool {
        self.is_on
    }

    pub fn update(&mut self, cur_ms: u128) -> bool {
        if cur_ms - self.last_blink > self.blink_rate {
            self.is_on = !self.is_on;
            self.last_blink = cur_ms;
            true
        } else {
            false
        }
    }
}

pub struct BufferView {
    pub buf: Buffer,
    sixel_cache: Vec<SixelCacheEntry>,
    pub caret: Caret,

    pub caret_blink: Blink,
    pub character_blink: Blink,

    pub scale: f32,
    pub scroll_back_line: i32,

    pub button_pressed: bool,

    pub selection_opt: Option<Selection>,

    program: glow::Program,
    vertex_array: glow::VertexArray,

    redraw_view: bool,

    redraw_palette: bool,
    colors: usize,

    redraw_font: bool,
    fonts: usize,

    font_texture: NativeTexture,
    buffer_texture: NativeTexture,
    palette_texture: NativeTexture,
    framebuffer: glow::NativeFramebuffer,
    render_texture: NativeTexture,
    render_buffer_size: Vec2,
    draw_program: glow::NativeProgram,
    sixel_shader: glow::NativeProgram,
    sixel_render_texture: NativeTexture,
}

impl BufferView {
    pub fn new(gl: &glow::Context) -> Self {
        let mut buf = Buffer::create(80, 25);
        buf.layers[0].is_transparent = false;
        buf.is_terminal_buffer = true;

        use glow::HasContext as _;

        unsafe {
            let sixel_shader = gl.create_program().expect("Cannot create program");
            let (vertex_shader_source, fragment_shader_source) = (
                r#"#version 330
const float low  =  -1.0;
const float high = 1.0;

const vec2 verts[6] = vec2[6](
    vec2(low, high),
    vec2(high, high),
    vec2(high, low),

    vec2(low, high),
    vec2(low, low),
    vec2(high, low)
);

void main() {
    vec2 vert = verts[gl_VertexID];
    gl_Position = vec4(vert, 0.3, 1.0);
}
"#,
                include_str!("sixel.shader.frag"),
            );
            let shader_sources = [
                (glow::VERTEX_SHADER, vertex_shader_source),
                (glow::FRAGMENT_SHADER, fragment_shader_source),
            ];

            let shaders: Vec<_> = shader_sources
                .iter()
                .map(|(shader_type, shader_source)| {
                    let shader = gl
                        .create_shader(*shader_type)
                        .expect("Cannot create shader");
                    gl.shader_source(
                        shader,
                        shader_source, /*&format!("{}\n{}", shader_version, shader_source)*/
                    );
                    gl.compile_shader(shader);
                    if !gl.get_shader_compile_status(shader) {
                        panic!("{}", gl.get_shader_info_log(shader));
                    }
                    gl.attach_shader(sixel_shader, shader);
                    shader
                })
                .collect();

            gl.link_program(sixel_shader);
            if !gl.get_program_link_status(sixel_shader) {
                panic!("{}", gl.get_program_info_log(sixel_shader));
            }

            for shader in shaders {
                gl.detach_shader(sixel_shader, shader);
                gl.delete_shader(shader);
            }

            let draw_program = gl.create_program().expect("Cannot create program");
            let (vertex_shader_source, fragment_shader_source) = (
                r#"#version 330
    const float low  =  -1.0;
    const float high = 1.0;
    
    const vec2 verts[6] = vec2[6](
        vec2(low, high),
        vec2(high, high),
        vec2(high, low),
    
        vec2(low, high),
        vec2(low, low),
        vec2(high, low)
    );
    
    void main() {
        vec2 vert = verts[gl_VertexID];
        gl_Position = vec4(vert, 0.3, 1.0);
    }
    "#,
                include_str!("render.shader.frag"),
            );
            let shader_sources = [
                (glow::VERTEX_SHADER, vertex_shader_source),
                (glow::FRAGMENT_SHADER, fragment_shader_source),
            ];

            let shaders: Vec<_> = shader_sources
                .iter()
                .map(|(shader_type, shader_source)| {
                    let shader = gl
                        .create_shader(*shader_type)
                        .expect("Cannot create shader");
                    gl.shader_source(
                        shader,
                        shader_source, /*&format!("{}\n{}", shader_version, shader_source)*/
                    );
                    gl.compile_shader(shader);
                    if !gl.get_shader_compile_status(shader) {
                        panic!("{}", gl.get_shader_info_log(shader));
                    }
                    gl.attach_shader(draw_program, shader);
                    shader
                })
                .collect();

            gl.link_program(draw_program);
            if !gl.get_program_link_status(draw_program) {
                panic!("{}", gl.get_program_info_log(draw_program));
            }

            for shader in shaders {
                gl.detach_shader(draw_program, shader);
                gl.delete_shader(shader);
            }

            let program = gl.create_program().expect("Cannot create program");

            let (vertex_shader_source, fragment_shader_source) = (
                r#"#version 330
const float low  =  -1.0;
const float high = 1.0;

const vec2 verts[6] = vec2[6](
    vec2(low, high),
    vec2(high, high),
    vec2(high, low),

    vec2(low, high),
    vec2(low, low),
    vec2(high, low)
);

void main() {
    vec2 vert = verts[gl_VertexID];
    gl_Position = vec4(vert, 0.3, 1.0);
}
"#,
                include_str!("buffer_view.shader.frag"),
            );
            let shader_sources = [
                (glow::VERTEX_SHADER, vertex_shader_source),
                (glow::FRAGMENT_SHADER, fragment_shader_source),
            ];

            let shaders: Vec<_> = shader_sources
                .iter()
                .map(|(shader_type, shader_source)| {
                    let shader = gl
                        .create_shader(*shader_type)
                        .expect("Cannot create shader");
                    gl.shader_source(
                        shader,
                        shader_source, /*&format!("{}\n{}", shader_version, shader_source)*/
                    );
                    gl.compile_shader(shader);
                    if !gl.get_shader_compile_status(shader) {
                        panic!("{}", gl.get_shader_info_log(shader));
                    }
                    gl.attach_shader(program, shader);
                    shader
                })
                .collect();

            gl.link_program(program);
            if !gl.get_program_link_status(program) {
                panic!("{}", gl.get_program_info_log(program));
            }

            for shader in shaders {
                gl.detach_shader(program, shader);
                gl.delete_shader(shader);
            }

            let vertex_array = gl
                .create_vertex_array()
                .expect("Cannot create vertex array");
            let buffer_texture = gl.create_texture().unwrap();
            create_buffer_texture(gl, &buf, 0, buffer_texture);
            gl.tex_parameter_i32(
                glow::TEXTURE_2D_ARRAY,
                glow::TEXTURE_MIN_FILTER,
                glow::NEAREST as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D_ARRAY,
                glow::TEXTURE_MAG_FILTER,
                glow::NEAREST as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D_ARRAY,
                glow::TEXTURE_WRAP_S,
                glow::CLAMP_TO_EDGE as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D_ARRAY,
                glow::TEXTURE_WRAP_T,
                glow::CLAMP_TO_EDGE as i32,
            );

            let palette_texture = gl.create_texture().unwrap();
            create_palette_texture(gl, &buf, palette_texture);
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

            let font_texture = gl.create_texture().unwrap();
            create_font_texture(gl, &buf, font_texture);
            gl.tex_parameter_i32(
                glow::TEXTURE_2D_ARRAY,
                glow::TEXTURE_MIN_FILTER,
                glow::NEAREST as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D_ARRAY,
                glow::TEXTURE_MAG_FILTER,
                glow::NEAREST as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D_ARRAY,
                glow::TEXTURE_WRAP_S,
                glow::CLAMP_TO_EDGE as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D_ARRAY,
                glow::TEXTURE_WRAP_T,
                glow::CLAMP_TO_EDGE as i32,
            );
            let colors = buf.palette.colors.len();
            let fonts = buf.font_table.len();
            let framebuffer = gl.create_framebuffer().unwrap();

            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(framebuffer));
            let render_texture = gl.create_texture().unwrap();
            let render_buffer_size = Vec2::new(
                buf.get_font_dimensions().width as f32 * buf.get_buffer_width() as f32,
                buf.get_font_dimensions().height as f32 * buf.get_buffer_height() as f32,
            );

            let filter = glow::NEAREST as i32;
            gl.bind_texture(glow::TEXTURE_2D, Some(render_texture));
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA as i32,
                render_buffer_size.x as i32,
                render_buffer_size.y as i32,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                None,
            );
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, filter);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, filter);
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

            let depth_buffer = gl.create_renderbuffer().unwrap();
            gl.bind_renderbuffer(glow::RENDERBUFFER, Some(depth_buffer));
            gl.renderbuffer_storage(
                glow::RENDERBUFFER,
                glow::DEPTH_COMPONENT,
                render_buffer_size.x as i32,
                render_buffer_size.y as i32,
            );
            gl.framebuffer_renderbuffer(
                glow::FRAMEBUFFER,
                glow::DEPTH_ATTACHMENT,
                glow::RENDERBUFFER,
                Some(depth_buffer),
            );
            gl.framebuffer_texture(
                glow::FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                Some(render_texture),
                0,
            );

            gl.bind_framebuffer(glow::FRAMEBUFFER, None);

            let sixel_render_texture = gl.create_texture().unwrap();
            let render_buffer_size = Vec2::new(
                buf.get_font_dimensions().width as f32 * buf.get_buffer_width() as f32,
                buf.get_font_dimensions().height as f32 * buf.get_buffer_height() as f32,
            );

            gl.bind_texture(glow::TEXTURE_2D, Some(sixel_render_texture));
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA as i32,
                render_buffer_size.x as i32,
                render_buffer_size.y as i32,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                None,
            );
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, filter);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, filter);
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

            let depth_buffer = gl.create_renderbuffer().unwrap();
            gl.bind_renderbuffer(glow::RENDERBUFFER, Some(depth_buffer));
            gl.renderbuffer_storage(
                glow::RENDERBUFFER,
                glow::DEPTH_COMPONENT,
                render_buffer_size.x as i32,
                render_buffer_size.y as i32,
            );
            gl.framebuffer_renderbuffer(
                glow::FRAMEBUFFER,
                glow::DEPTH_ATTACHMENT,
                glow::RENDERBUFFER,
                Some(depth_buffer),
            );
            gl.framebuffer_texture(
                glow::FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                Some(sixel_render_texture),
                0,
            );

            gl.bind_framebuffer(glow::FRAMEBUFFER, None);

            Self {
                buf,
                caret: Caret::default(),
                caret_blink: Blink::new((1000.0 / 1.875) as u128 / 2),
                character_blink: Blink::new((1000.0 / 1.8) as u128),
                scale: 1.0,
                sixel_cache: Vec::new(),
                button_pressed: false,
                redraw_view: false,
                redraw_palette: false,
                redraw_font: false,
                scroll_back_line: 0,
                selection_opt: None,
                colors,
                fonts,
                program,
                draw_program,
                vertex_array,
                font_texture,
                buffer_texture,
                palette_texture,

                framebuffer,
                render_texture,
                render_buffer_size,

                sixel_shader,
                sixel_render_texture,
            }
        }
    }

    pub fn redraw_view(&mut self) {
        self.redraw_view = true;
    }

    pub fn _destroy(&self, gl: &glow::Context) {
        use glow::HasContext as _;
        unsafe {
            gl.delete_program(self.program);
            gl.delete_vertex_array(self.vertex_array);
        }
    }
}
