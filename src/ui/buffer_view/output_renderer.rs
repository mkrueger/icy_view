use eframe::egui;
use egui::PaintCallbackInfo;
use egui::Rect;
use egui::Vec2;
use glow::HasContext as _;
use glow::NativeTexture;
use icy_engine::Buffer;

pub struct OutputRenderer {
    output_shader: glow::NativeProgram,

    pub framebuffer: glow::NativeFramebuffer,
    pub render_texture: NativeTexture,
    pub render_buffer_size: Vec2,
    pub vertex_array: glow::VertexArray,
}

impl OutputRenderer {
    pub fn new(gl: &glow::Context, buf: &Buffer, filter: i32) -> Self {
        unsafe {
            let render_buffer_size = Vec2::new(
                buf.get_font_dimensions().width as f32 * buf.get_buffer_width() as f32,
                buf.get_font_dimensions().height as f32 * buf.get_buffer_height() as f32,
            );

            let output_shader = compile_output_shader(gl);
            let framebuffer = gl.create_framebuffer().unwrap();
            let render_texture = create_screen_render_texture(gl, render_buffer_size, filter);
            let vertex_array = gl
                .create_vertex_array()
                .expect("Cannot create vertex array");
            Self {
                output_shader,
                framebuffer,
                render_texture,
                render_buffer_size,
                vertex_array,
            }
        }
    }

    pub fn destroy(&self, gl: &glow::Context) {
        unsafe {
            gl.delete_program(self.output_shader);
            gl.delete_vertex_array(self.vertex_array);
            gl.delete_texture(self.render_texture);
            gl.delete_framebuffer(self.framebuffer);
        }
    }

    pub(crate) unsafe fn init_output(&self, gl: &glow::Context) {
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.framebuffer));
        gl.framebuffer_texture(
            glow::FRAMEBUFFER,
            glow::COLOR_ATTACHMENT0,
            Some(self.render_texture),
            0,
        );
        gl.bind_texture(glow::TEXTURE_2D, Some(self.render_texture));
        gl.viewport(
            0,
            0,
            self.render_buffer_size.x as i32,
            self.render_buffer_size.y as i32,
        );
        gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);
        gl.clear_color(0., 0., 0., 1.0);
    }

    pub unsafe fn render_to_screen(
        &self,
        gl: &glow::Context,
        info: &PaintCallbackInfo,
        output_texture: glow::NativeTexture,
        rect: Rect,
    ) {
        gl.bind_framebuffer(glow::FRAMEBUFFER, None);
        gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);

        gl.viewport(
            info.clip_rect.left() as i32,
            (info.screen_size_px[1] as f32 - info.clip_rect.max.y * info.pixels_per_point) as i32,
            (info.viewport.width() * info.pixels_per_point) as i32,
            (info.viewport.height() * info.pixels_per_point) as i32,
        );
        gl.use_program(Some(self.output_shader));
        gl.active_texture(glow::TEXTURE0);
        gl.uniform_1_i32(
            gl.get_uniform_location(self.output_shader, "u_render_texture")
                .as_ref(),
            0,
        );
        gl.bind_texture(glow::TEXTURE_2D, Some(output_texture));

        gl.uniform_1_f32(
            gl.get_uniform_location(self.output_shader, "u_effect")
                .as_ref(),
            0.0,
        );

        gl.uniform_1_f32(
            gl.get_uniform_location(self.output_shader, "u_use_monochrome")
                .as_ref(),
            0.0,
        );

        gl.uniform_2_f32(
            gl.get_uniform_location(self.output_shader, "u_resolution")
                .as_ref(),
            rect.width() * info.pixels_per_point,
            rect.height() * info.pixels_per_point,
        );
        gl.uniform_2_f32(
            gl.get_uniform_location(self.output_shader, "u_position")
                .as_ref(),
            rect.left() * info.pixels_per_point,
            rect.top() * info.pixels_per_point,
        );
        gl.uniform_4_f32(
            gl.get_uniform_location(self.output_shader, "u_draw_rect")
                .as_ref(),
            info.clip_rect.left() * info.pixels_per_point,
            info.clip_rect.top() * info.pixels_per_point,
            info.clip_rect.width() * info.pixels_per_point,
            info.clip_rect.height() * info.pixels_per_point,
        );

        gl.uniform_4_f32(
            gl.get_uniform_location(self.output_shader, "u_draw_area")
                .as_ref(),
            (rect.left() - 3.) * info.pixels_per_point,
            (rect.top() - info.clip_rect.top() - 4.) * info.pixels_per_point,
            (rect.right() - 3.) * info.pixels_per_point,
            (rect.bottom() - info.clip_rect.top() - 4.) * info.pixels_per_point,
        );

        gl.uniform_2_f32(
            gl.get_uniform_location(self.output_shader, "u_size")
                .as_ref(),
            rect.width() * info.pixels_per_point,
            rect.height() * info.pixels_per_point,
        );

        gl.bind_vertex_array(Some(self.vertex_array));
        gl.draw_arrays(glow::TRIANGLES, 0, 3);
        gl.draw_arrays(glow::TRIANGLES, 3, 3);
    }

    pub(crate) fn update_render_buffer(
        &mut self,
        gl: &glow::Context,
        buf: &Buffer,
        scale_filter: i32,
    ) {
        let render_buffer_size = Vec2::new(
            buf.get_font_dimensions().width as f32 * buf.get_buffer_width() as f32,
            buf.get_font_dimensions().height as f32 * buf.get_buffer_height() as f32,
        );
        if render_buffer_size == self.render_buffer_size {
            return;
        }
        unsafe {
            use glow::HasContext as _;
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.framebuffer));
            gl.delete_texture(self.render_texture);

            let render_texture = gl.create_texture().unwrap();
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
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, scale_filter);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, scale_filter);
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
            self.render_texture = render_texture;
            self.render_buffer_size = render_buffer_size;
        }
    }
}

unsafe fn compile_output_shader(gl: &glow::Context) -> glow::NativeProgram {
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
        include_str!("output_renderer.shader.frag"),
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
            assert!(
                gl.get_shader_compile_status(shader),
                "{}",
                gl.get_shader_info_log(shader)
            );
            gl.attach_shader(draw_program, shader);
            shader
        })
        .collect();

    gl.link_program(draw_program);
    assert!(
        gl.get_program_link_status(draw_program),
        "{}",
        gl.get_program_info_log(draw_program)
    );

    for shader in shaders {
        gl.detach_shader(draw_program, shader);
        gl.delete_shader(shader);
    }
    draw_program
}

unsafe fn create_screen_render_texture(
    gl: &glow::Context,
    render_buffer_size: Vec2,
    filter: i32,
) -> NativeTexture {
    let render_texture = gl.create_texture().unwrap();
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
    render_texture
}
