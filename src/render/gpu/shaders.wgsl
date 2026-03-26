const MAX_JOINTS: u32 = 512;

struct Uniforms {
    mvp_matrix: mat4x4<f32>,
    model_matrix: mat4x4<f32>,
    normal_matrix: mat3x4<f32>,
    camera_pos: vec4<f32>,
    light_dir: vec4<f32>,
    lighting_params: vec4<f32>,
    material_color: vec4<f32>,
    fog_params: vec4<f32>,
    uv_transform: vec4<f32>,
    uv_params: vec4<f32>,
    alpha_params: vec4<f32>,
    texture_params: vec4<f32>,
    exposure: f32,
    has_skin: u32,
    _pad: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(0) @binding(1)
var<storage, read> joint_matrices: array<mat4x4<f32>, MAX_JOINTS>;

@group(1) @binding(0)
var diffuse_texture: texture_2d<f32>;

@group(1) @binding(1)
var diffuse_sampler: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv0: vec2<f32>,
    @location(3) uv1: vec2<f32>,
    @location(4) joint_indices: vec4<u32>,
    @location(5) joint_weights: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_normal: vec3<f32>,
    @location(1) world_position: vec3<f32>,
    @location(2) uv0: vec2<f32>,
    @location(3) uv1: vec2<f32>,
    @location(4) view_depth: f32,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    
    var skinned_position = input.position;
    var skinned_normal = input.normal;
    
    if (uniforms.has_skin > 0u) {
        var skin_matrix: mat4x4<f32> = mat4x4<f32>(
            vec4<f32>(0.0, 0.0, 0.0, 0.0),
            vec4<f32>(0.0, 0.0, 0.0, 0.0),
            vec4<f32>(0.0, 0.0, 0.0, 0.0),
            vec4<f32>(0.0, 0.0, 0.0, 0.0)
        );
        var accumulated_weight: f32 = 0.0;
        
        if (input.joint_indices.x < MAX_JOINTS) {
            skin_matrix = skin_matrix + joint_matrices[input.joint_indices.x] * input.joint_weights.x;
            accumulated_weight = accumulated_weight + input.joint_weights.x;
        }
        if (input.joint_indices.y < MAX_JOINTS) {
            skin_matrix = skin_matrix + joint_matrices[input.joint_indices.y] * input.joint_weights.y;
            accumulated_weight = accumulated_weight + input.joint_weights.y;
        }
        if (input.joint_indices.z < MAX_JOINTS) {
            skin_matrix = skin_matrix + joint_matrices[input.joint_indices.z] * input.joint_weights.z;
            accumulated_weight = accumulated_weight + input.joint_weights.z;
        }
        if (input.joint_indices.w < MAX_JOINTS) {
            skin_matrix = skin_matrix + joint_matrices[input.joint_indices.w] * input.joint_weights.w;
            accumulated_weight = accumulated_weight + input.joint_weights.w;
        }
        
        let skinned = skin_matrix * vec4<f32>(input.position, 1.0);
        if (accumulated_weight > 1e-6 && abs(skinned.w) > 1e-6) {
            skinned_position = skinned.xyz / skinned.w;
        } else {
            skinned_position = input.position;
        }
        skinned_normal = normalize((skin_matrix * vec4<f32>(input.normal, 0.0)).xyz);
    }
    
    let world_pos = uniforms.model_matrix * vec4<f32>(skinned_position, 1.0);
    output.clip_position = uniforms.mvp_matrix * vec4<f32>(skinned_position, 1.0);
    output.world_normal = (uniforms.normal_matrix * skinned_normal).xyz;
    output.world_position = world_pos.xyz;
    output.uv0 = input.uv0;
    output.uv1 = input.uv1;
    output.view_depth = (output.clip_position.z / output.clip_position.w + 1.0) * 0.5;
    
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let ambient_strength = uniforms.lighting_params.x;
    let diffuse_strength = uniforms.lighting_params.y;
    let specular_strength = uniforms.lighting_params.z;
    let specular_power = uniforms.lighting_params.w;
    let rim_strength = uniforms.fog_params.w;

    let normal = normalize(input.world_normal);
    let light_dir = normalize(uniforms.light_dir.xyz);

    let use_uv1 = uniforms.uv_params.x > 0.5;
    let uv_source = select(input.uv0, input.uv1, use_uv1);
    let angle = uniforms.uv_params.y;
    let sin_t = sin(angle);
    let cos_t = cos(angle);
    let scaled_uv = vec2<f32>(
        uv_source.x * uniforms.uv_transform.z,
        uv_source.y * uniforms.uv_transform.w,
    );
    let rotated_uv = vec2<f32>(
        scaled_uv.x * cos_t - scaled_uv.y * sin_t,
        scaled_uv.x * sin_t + scaled_uv.y * cos_t,
    );
    let sample_uv = vec2<f32>(
        rotated_uv.x + uniforms.uv_transform.x,
        rotated_uv.y + uniforms.uv_transform.y,
    );
    let use_legacy_v_origin = uniforms.uv_params.z > 0.5;
    let resolved_uv = select(sample_uv, vec2<f32>(sample_uv.x, 1.0 - sample_uv.y), use_legacy_v_origin);

    let max_mip = uniforms.texture_params.z;
    let depth_lod = clamp(input.view_depth, 0.0, 1.0);
    let lod = clamp(
        depth_lod * 6.0 + uniforms.texture_params.x + uniforms.texture_params.y,
        0.0,
        max_mip,
    );
    let base_color = textureSampleLevel(diffuse_texture, diffuse_sampler, resolved_uv, lod);
    let material_tint = uniforms.material_color.rgb;
    let surface_color = base_color.rgb * material_tint;

    let ambient = ambient_strength * surface_color;

    let diff = max(dot(normal, light_dir), 0.0);
    let diffuse = diffuse_strength * diff * surface_color;

    let view_dir = normalize(uniforms.camera_pos.xyz - input.world_position);
    let reflect_dir = reflect(-light_dir, normal);
    let spec = pow(max(dot(view_dir, reflect_dir), 0.0), specular_power);
    let specular = specular_strength * spec;

    let rim_factor = 1.0 - max(dot(view_dir, normal), 0.0);
    let rim = rim_strength * pow(rim_factor, 2.0) * surface_color;

    let fog_near = uniforms.fog_params.x;
    let fog_far = uniforms.fog_params.y;
    let fog_strength = uniforms.fog_params.z;
    let depth = input.view_depth;
    let fog_factor = 1.0 - min(1.0, fog_strength * smoothstep(fog_near, fog_far, depth));

    let color = (ambient + diffuse + specular + rim) * fog_factor;
    let alpha = base_color.a * uniforms.material_color.a;

    let alpha_mode = u32(uniforms.alpha_params.x);
    let alpha_cutoff = uniforms.alpha_params.y;
    if (alpha_mode == 1u && alpha < alpha_cutoff) {
        discard;
    }

    let exposed_color = color * uniforms.exposure;
    
    return vec4<f32>(exposed_color, alpha);
}
