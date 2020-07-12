#version 330 core

layout (location = 0) in vec3 position;
layout (location = 1) in vec3 tangent;
layout (location = 2) in vec3 bitangent;
layout (location = 3) in vec3 normal;
layout (location = 4) in vec2 uv;

out vec4 f_tangent;
out vec4 f_bitangent;
out vec4 f_normal;
out vec2 f_uvs;

uniform mat4 mvp;
uniform mat4 model_matrix;

void main() {
    f_tangent = model_matrix * vec4(tangent, 0.0);
    f_bitangent = model_matrix * vec4(bitangent, 0.0);
    f_normal = model_matrix * vec4(normal, 0.0);
    f_uvs = uv;
    gl_Position = mvp * vec4(position, 1.0);
}