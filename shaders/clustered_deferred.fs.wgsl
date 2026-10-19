// G-buffer pass: stores what the fullscreen pass needs for lighting, without shading anything.

@group(${bindGroup_material}) @binding(0) var diffuseTex: texture_2d<f32>;
@group(${bindGroup_material}) @binding(1) var diffuseTexSampler: sampler;

struct FragmentInput
{
    @location(0) pos: vec3f,
    @location(1) nor: vec3f,
    @location(2) uv: vec2f
}

struct GBufferOutput
{
    // alpha 1 marks the pixel as covered; the G-buffer is cleared to 0
    @location(0) albedo: vec4f,
    @location(1) normal: vec4f,
    @location(2) position: vec4f,
}

@fragment
fn main(in: FragmentInput) -> GBufferOutput
{
    let diffuseColor = textureSample(diffuseTex, diffuseTexSampler, in.uv);
    if (diffuseColor.a < 0.5f) {
        discard;
    }

    var out: GBufferOutput;
    out.albedo = vec4f(diffuseColor.rgb, 1.0);
    out.normal = vec4f(normalize(in.nor), 0.0);
    out.position = vec4f(in.pos, 1.0);
    return out;
}
