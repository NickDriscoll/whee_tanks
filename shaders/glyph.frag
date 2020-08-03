#version 330 core

in vec2 f_uvs;

out vec4 frag_color;

uniform sampler2D glyph_texture;

void main() {
    float intensity = texture(glyph_texture, f_uvs).r;
    frag_color = vec4(intensity, intensity, intensity, intensity);
}