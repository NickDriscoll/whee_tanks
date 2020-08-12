#version 330 core

in vec2 position;

uniform mat4 clipping_from_screen;

void main() {
    gl_Position = clipping_from_screen * vec4(position, 0.1, 1.0);
}