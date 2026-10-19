@group(${bindGroup_gBuffer}) @binding(0) var gBufferAlbedo: texture_2d<f32>;
@group(${bindGroup_gBuffer}) @binding(1) var gBufferNormal: texture_2d<f32>;
@group(${bindGroup_gBuffer}) @binding(2) var gBufferPosition: texture_2d<f32>;

@fragment
fn main(@builtin(position) fragPos: vec4f) -> @location(0) vec4f
{
    let pixel = vec2i(fragPos.xy);
    let albedo = textureLoad(gBufferAlbedo, pixel, 0);
    if (albedo.a < 0.5) {
        discard;
    }

    let nor = normalize(textureLoad(gBufferNormal, pixel, 0).xyz);
    let posWorld = textureLoad(gBufferPosition, pixel, 0).xyz;
    return shadeClustered(posWorld, nor, albedo.rgb);
}
