// Prepended (after common.wgsl) to the Forward+ and Clustered Deferred fullscreen fragment
// shaders, which both read the clustering output through these bindings.

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

// nor must be normalized.
fn shadeClustered(posWorld: vec3f, nor: vec3f, albedo: vec3f) -> vec4f {
    let clusterCoords = clusterCoordsAt(posWorld, cameraUniforms.viewProj, cameraUniforms.view);
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

    let totalLightContrib = clusterLightContrib(cluster, posWorld, nor);

    if (debugView == 3u) {
        let diff = abs(totalLightContrib - allLightsContrib(posWorld, nor));
        return vec4f(diff * 1000.0, 1.0);
    }

    return vec4f(albedo * totalLightContrib, 1.0);
}
