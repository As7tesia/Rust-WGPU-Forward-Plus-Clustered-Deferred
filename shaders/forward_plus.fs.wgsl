// TODO-2: implement the Forward+ fragment shader

// See naive.fs.wgsl for basic fragment shader setup; this shader should use light clusters instead of looping over all lights

// ------------------------------------
// Shading process:
// ------------------------------------
// Determine which cluster contains the current fragment.
// Retrieve the number of lights that affect the current fragment from the cluster’s data.
// Initialize a variable to accumulate the total light contribution for the fragment.
// For each light in the cluster:
//     Access the light's properties using its index.
//     Calculate the contribution of the light based on its position, the fragment’s position, and the surface normal.
//     Add the calculated contribution to the total light accumulation.
// Multiply the fragment’s diffuse color by the accumulated light contribution.
// Return the final color, ensuring that the alpha component is set appropriately (typically to 1).

@group(${bindGroup_scene}) @binding(0) var<uniform> cameraUniforms: CameraUniforms;
@group(${bindGroup_scene}) @binding(1) var<storage, read> lightSet: LightSet;
@group(${bindGroup_scene}) @binding(2) var<storage, read> clusterCounts: array<u32>;
@group(${bindGroup_scene}) @binding(3) var<storage, read> clusterLightIndices: array<u32>;

// 0: lighting
// 1: lights per cluster: black = none, blue to red = up to debugCountScale, magenta = over the cap
// 2: cluster coordinates: red = x (left to right), green = y (bottom to top), blue = z slice
// 3: |clustered - all lights| * 1000: black where the cluster has every light that reaches the pixel
const debugView = 0u;
const debugCountScale = 128.0;

@group(${bindGroup_material}) @binding(0) var diffuseTex: texture_2d<f32>;
@group(${bindGroup_material}) @binding(1) var diffuseTexSampler: sampler;

fn clusterLightContrib(cluster: u32, posWorld: vec3f, nor: vec3f) -> vec3f {
    let numLights = min(clusterCounts[cluster], maxLightsPerCluster);
    let base = cluster * maxLightsPerCluster;
    var total = vec3f(0.0);
    for (var i = 0u; i < numLights; i++) {
        total += calculateLightContrib(lightSet.lights[clusterLightIndices[base + i]], posWorld, nor);
    }
    return total;
}

fn allLightsContrib(posWorld: vec3f, nor: vec3f) -> vec3f {
    var total = vec3f(0.0);
    for (var i = 0u; i < lightSet.numLights; i++) {
        total += calculateLightContrib(lightSet.lights[i], posWorld, nor);
    }
    return total;
}

struct FragmentInput
{
    @location(0) pos: vec3f,
    @location(1) nor: vec3f,
    @location(2) uv: vec2f
}

@fragment
fn main(in: FragmentInput) -> @location(0) vec4f
{
    let diffuseColor = textureSample(diffuseTex, diffuseTexSampler, in.uv);
    if (diffuseColor.a < 0.5f) {
        discard;
    }

    let clusterCoords = clusterCoordsAt(in.pos, cameraUniforms.viewProj, cameraUniforms.view);
    let cluster = clusterIndex(clusterCoords.x, clusterCoords.y, clusterCoords.z);

    if (debugView == 1u) {
        let count = clusterCounts[cluster];
        if (count > maxLightsPerCluster) {
            return vec4f(1.0, 0.0, 1.0, 1.0);
        }
        if (count == 0u) {
            return vec4f(0.0, 0.0, 0.0, 1.0);
        }
        let t = min(f32(count) / debugCountScale, 1.0);
        return vec4f(t, 0.0, 1.0 - t, 1.0);
    }
    if (debugView == 2u) {
        return vec4f(vec3f(clusterCoords) / vec3f(gridRes - 1u), 1.0);
    }

    let nor = normalize(in.nor);
    let totalLightContrib = clusterLightContrib(cluster, in.pos, nor);

    if (debugView == 3u) {
        let diff = abs(totalLightContrib - allLightsContrib(in.pos, nor));
        return vec4f(diff * 1000.0, 1.0);
    }

    return vec4f(diffuseColor.rgb * totalLightContrib, 1.0);
}

