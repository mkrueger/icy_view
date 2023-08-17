#version 330
in vec2 UV;

uniform sampler2D u_render_texture;
uniform vec2      u_resolution;
uniform vec2      u_position;
uniform float     u_effect;
uniform vec4      u_draw_rect;
uniform vec4      u_draw_area;
uniform vec2      u_size;

uniform float gamma;
uniform float contrast;
uniform float saturation;
uniform float brightness;
uniform float curvature;
uniform float light;
uniform float blur;
uniform float u_scanlines;
uniform float u_use_monochrome;
uniform vec3  u_monchrome_mask;

out vec3 color;

// Shader used: 
// https://www.shadertoy.com/view/XdyGzR

vec3 postEffects(in vec3 rgb, in vec2 xy) {
    rgb = pow(rgb, vec3(gamma));
    rgb = mix(vec3(.5), mix(vec3(dot(vec3(.2125, .7154, .0721), rgb * brightness)), rgb * brightness, saturation), contrast);
    return rgb;
}

// Sigma 1. Size 3
vec3 gaussian(in vec2 uv) {
    float b = blur / (u_size.x / u_size.y);

    uv+= .5;

    vec3 col = texture(u_render_texture, vec2(uv.x - b/u_size.x, uv.y - b/u_size.y) ).rgb * 0.077847;
    col += texture(u_render_texture, vec2(uv.x - b/u_size.x, uv.y) ).rgb * 0.123317;
    col += texture(u_render_texture, vec2(uv.x - b/u_size.x, uv.y + b/u_size.y) ).rgb * 0.077847;

    col += texture(u_render_texture, vec2(uv.x, uv.y - b/u_size.y) ).rgb * 0.123317;
    col += texture(u_render_texture, vec2(uv.x, uv.y) ).rgb * 0.195346;
    col += texture(u_render_texture, vec2(uv.x, uv.y + b/u_size.y) ).rgb * 0.123317;

    col += texture(u_render_texture, vec2(uv.x + b/u_size.x, uv.y - b/u_size.y) ).rgb * 0.077847;
    col += texture(u_render_texture, vec2(uv.x + b/u_size.x, uv.y) ).rgb * 0.123317;
    col += texture(u_render_texture, vec2(uv.x + b/u_size.x, uv.y + b/u_size.y) ).rgb * 0.077847;

    return col;
}

void scanlines2(vec2 coord)
{
	vec2 st = coord - vec2(.5);
    // Curvature/light
    float d = length(st *.5 * st *.5 * curvature);
    vec2 uv = st * d + st;

    // Fudge aspect ratio
#ifdef ASPECT_RATIO
    uv.x *= u_size.x/u_size.y*.75;
#endif
    
    // CRT color blur
    vec3 col = gaussian(uv);

    // Light
	if (light > 0.0) {
    	float l = 1. - min(1., d * light);
    	col *= l;
	}

    // Scanlines
    float y = uv.y;

    float showScanlines = 1.;
    if (u_size.y < 360.) {
		showScanlines = 0.;
	}
    
	float s = 1. - smoothstep(320., 1440., u_size.y) + 1.;
	float j = cos(y*u_size.y*s)*u_scanlines; // values between .01 to .25 are ok.
	col = abs(showScanlines - 1.)*col + showScanlines * (col - col*j);
	col *= 1. - ( .01 + ceil(mod( (st.x+.5)*u_size.x, 3.) ) * (.995-1.01) )*showScanlines;

    // Border mask
	if (curvature > 0.0) {
		float m = max(0.0, 1. - 2. * max(abs(uv.x), abs(uv.y) ));
		m = min(m * 200., 1.);
		col *= m;
	}

    color = postEffects(col, st);
}

void main() {
	vec2 uv   = (gl_FragCoord.xy - u_draw_rect.xy) / u_draw_rect.zw;
	vec2 from = u_draw_area.xy / u_draw_rect.zw;
	vec2 to   = u_draw_area.zw / u_draw_rect.zw;

	if (from.x <= uv.x && uv.x < to.x && 
		from.y <= uv.y && uv.y < to.y) {
		color = texture(u_render_texture, (uv - from) / (to - from)).xyz;
	}
}    