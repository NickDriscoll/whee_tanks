#version 330 core

out vec4 frag_color;

uniform vec4 button_color = vec4(0.0, 0.0, 0.0, 0.5);

void main() {
    frag_color = button_color;
}