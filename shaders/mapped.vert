#version 330 core

layout (location = 0) in vec3 position;
layout (location = 1) in vec3 tangent;
layout (location = 2) in vec3 bitangent;
layout (location = 3) in vec3 normal;
layout (location = 4) in vec2 uv;

out mat3 tangent_matrix;
out vec4 shadow_space_pos;
out vec2 f_uvs;

uniform mat4 mvp;
uniform mat4 model_matrix;
uniform mat4 shadow_matrix;

void main() {
    mat4 normal_matrix = transpose(mat4(inverse(mat3(model_matrix))));
    vec3 T = normalize(vec3(normal_matrix * vec4(tangent, 0.0)));
    vec3 B = normalize(vec3(normal_matrix * vec4(bitangent, 0.0)));
    vec3 N = normalize(vec3(normal_matrix * vec4(normal, 0.0)));
    tangent_matrix = mat3(T, B, N);

    shadow_space_pos = shadow_matrix * model_matrix * vec4(position, 1.0);

    f_uvs = uv;
    gl_Position = mvp * vec4(position, 1.0);
}