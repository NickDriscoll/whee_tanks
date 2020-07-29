#version 330 core

in mat3 tangent_matrix;
in vec2 f_uvs;

out vec4 frag_color;

uniform sampler2D albedo_map;
uniform sampler2D normal_map;
uniform sampler2D roughness_map;

uniform vec4 sun_direction;
uniform vec4 view_direction;

const float AMBIENT = 0.1;

void main() {
    vec3 albedo = texture(albedo_map, f_uvs).xyz;
    vec3 tangent_normal = texture(normal_map, f_uvs).xyz * 2.0 - 1.0;
    //vec3 normal = normalize(tangent_matrix * tangent_normal);
    vec3 normal = normalize(tangent_matrix[2]);

    float diffuse = max(0.0, dot(vec3(sun_direction), normal));

    float roughness = texture(roughness_map, f_uvs).x;
    vec4 halfway = normalize(view_direction + sun_direction);
    float specular_angle = max(0.0, dot(vec3(halfway), normal));
    float specular_coefficient = 100 / (500 * roughness + 0.01);
    float specular = pow(specular_angle, specular_coefficient);

    //frag_color = vec4(normal / 2.0 + 0.5, 1.0);
    frag_color = vec4((specular + diffuse + AMBIENT) * albedo, 1.0);
}