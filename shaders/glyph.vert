#version 330 core

in vec2 position;
in vec2 uvs;

out vec2 f_uvs;

uniform mat4 clipping_from_screen;

void main() {
    f_uvs = uvs;
    gl_Position = clipping_from_screen * vec4(position, 0.0, 1.0);
}