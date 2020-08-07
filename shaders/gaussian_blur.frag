#version 330 core

in vec2 f_uvs;

out vec4 frag_color;

uniform sampler2D image_texture;
uniform bool horizontal;

const int TAP = 7; //Must be odd
const int BOUND = TAP / 2;
const float GAUSSIAN_WEIGHTS[TAP / 2 + 1] = float[](0.2346368, 0.2011173, 0.1256984, 0.0558159);

void main() {
    vec2 texel_size = 1.0 / textureSize(image_texture, 0);
    vec3 result = vec3(0.0);
    if (horizontal) {
        for (int i = -BOUND; i <= BOUND; i++) {
            float weight = GAUSSIAN_WEIGHTS[abs(i)];
            vec3 sample = texture(image_texture, f_uvs + vec2(i, 0) * texel_size).xyz;
            result += sample * weight;
        }
    } else {
        for (int i = -BOUND; i <= BOUND; i++) {
            float weight = GAUSSIAN_WEIGHTS[abs(i)];
            vec3 sample = texture(image_texture, f_uvs + vec2(0, i) * texel_size).xyz;
            result += sample * weight;
        }
    }
    frag_color = vec4(result, 1.0);
}