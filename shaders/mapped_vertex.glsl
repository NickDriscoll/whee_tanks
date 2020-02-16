#version 330 core

in vec3 position;
in vec2 uv;
out vec4 f_normal;
out vec2 f_uvs;

uniform mat4 mvp;
uniform mat4 model_matrix;

void main() {
    f_uvs = uv;
    gl_Position = mvp * vec4(position, 1.0);
}