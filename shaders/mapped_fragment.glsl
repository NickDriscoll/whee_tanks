#version 330 core

in vec3 f_tangent;
in vec3 f_bitangent;
in vec3 f_normal;
in vec2 f_uvs;

out vec4 frag_color;

uniform sampler2D albedo_map;
uniform sampler2D normal_map;
uniform vec4 sun_direction;

void main() {
    mat4 tangent_matrix = mat4(
        f_tangent.x, f_bitangent.x, f_normal.x, 0.0,
        f_tangent.y, f_bitangent.y, f_normal.y, 0.0,
        f_tangent.z, f_bitangent.z, f_normal.z, 0.0,
        0.0, 0.0, 0.0, 1.0
    );

    vec4 normal = texture(normal_map, f_uvs) * 2 - 1;

    frag_color = texture(albedo_map, f_uvs);
}