#version 330 core

in mat3 tangent_matrix;
in vec2 f_uvs;

out vec4 frag_color;

uniform sampler2D albedo_map;
uniform sampler2D normal_map;
uniform vec4 sun_direction;

const float AMBIENT = 0.2;

void main() {
    vec3 albedo = texture(albedo_map, f_uvs).xyz;
    vec3 normal = normalize(tangent_matrix * (texture(normal_map, f_uvs).xyz * 2.0 - 1.0));
    float diffuse = max(0.0, dot(vec3(sun_direction), normal));

    frag_color = vec4(normal / 2.0 + 0.5, 1.0);
    //frag_color = vec4((diffuse + AMBIENT) * albedo, 1.0);
}