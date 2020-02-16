#version 330 core

in vec3 position;
in vec3 normal;
in vec2 uv;
out vec4 f_normal;
out vec2 f_uvs;

uniform mat4 mvp;
uniform mat4 model_matrix;

void main() {
    f_uvs = uv;
    
    //Send world space representation of normal vector
	//We use a normal matrix instead of just the model matrix so that non-uniform scaling doesn't mess up the normal vector
	mat4 normal_matrix = transpose(mat4(inverse(mat3(model_matrix))));
	f_normal = normal_matrix * vec4(normal, 0.0);

    gl_Position = mvp * vec4(position, 1.0);
}