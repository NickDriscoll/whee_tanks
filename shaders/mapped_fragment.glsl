#version 330 core

in vec2 f_uvs;
out vec4 frag_color;

uniform sampler2D albedo;

void main() {
    frag_color = texture(albedo, f_uvs);
}