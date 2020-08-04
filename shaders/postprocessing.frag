#version 330 core

in vec2 f_uvs;

out vec4 frag_color;

uniform sampler2D image_texture;

void main() {
    frag_color = vec4(texture(image_texture, f_uvs).xyz, 1.0);
}