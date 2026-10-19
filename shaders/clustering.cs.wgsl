// TODO-2: implement the light clustering compute shader
//
// Scatter: one thread per light, appending the light to every cluster its sphere touches. The
// counts are zeroed by `clear_buffer` before this pass (see `Lights::do_light_clustering`).
//
// For each light:
//     - Move its position into view space.
//     - Find the range of clusters (x, y, z) its sphere can touch, clamped to the grid.
//     - For each cluster in that range, atomicAdd the cluster's count to get a slot, and write
//       the light's index there if the slot is below maxLightsPerCluster.
//
// The ranges come from a box around the sphere, so a cluster can receive a light that doesn't
// actually reach it. That costs shading time but never drops a light.

@group(${bindGroup_scene}) @binding(0) var<uniform> cameraUniforms: CameraUniforms;
@group(${bindGroup_scene}) @binding(1) var<storage, read> lightSet: LightSet;
@group(${bindGroup_scene}) @binding(2) var<storage, read_write> clusterCounts: array<atomic<u32>>;
@group(${bindGroup_scene}) @binding(3) var<storage, read_write> clusterLightIndices: array<u32>;

@compute
@workgroup_size(${clusterLightsWorkgroupSize})
fn main(@builtin(global_invocation_id) globalIdx: vec3u) {
    let lightIdx = globalIdx.x;
    if (lightIdx >= lightSet.numLights) {
        return;
    }
    
    // View-space position and depth
    let viewPos = (cameraUniforms.view * vec4f(lightSet.lights[lightIdx].pos, 1.0)).xyz;
    let depth = -viewPos.z; // view space looks down -z, so depth in front of the camera is -z
    let r = lightRadius;

    if (depth + r < clusterNear || depth - r > clusterFar) {
        return;
    }

    // slices that depth - r and depth + r crosses are the target z index
    let zMin = sliceFromDepth(depth - r);
    let zMax = sliceFromDepth(depth + r);

    // X/Y range 
    // whole screen is the fallback when the sphere reaches behind the camera
    var gridMin = vec2u(0u, 0u);
    var gridMax = gridRes.xy - vec2u(1u);
    if (depth - r > 0.0) {
        // project 8 corners of the sphere's view-space box and take the NDC rectangle around them.
        var ndcMin = vec2f(1e30);
        var ndcMax = vec2f(-1e30);
        for (var i = 0u; i < 8u; i++) {
            // bits 0, 1, 2 of i pick the -r or +r side on x, y, z
            let corner = viewPos + r * vec3f(
                select(-1.0, 1.0, (i & 1u) != 0u),
                select(-1.0, 1.0, (i & 2u) != 0u),
                select(-1.0, 1.0, (i & 4u) != 0u),
            );
            let clip = cameraUniforms.proj * vec4f(corner, 1.0);
            let ndc = clip.xy / clip.w;
            ndcMin = min(ndcMin, ndc);
            ndcMax = max(ndcMax, ndc);
        }

        // entirely off screen
        if (any(ndcMax < vec2f(-1.0)) || any(ndcMin > vec2f(1.0))) {
            return;
        }

        gridMin = gridXYFromNdc(ndcMin);
        gridMax = gridXYFromNdc(ndcMax);
    }
    let xMin = gridMin.x;
    let xMax = gridMax.x;
    let yMin = gridMin.y;
    let yMax = gridMax.y;

    for (var z = zMin; z <= zMax; z++) {
        for (var y = yMin; y <= yMax; y++) {
            for (var x = xMin; x <= xMax; x++) {
                let index = clusterIndex(x, y, z);
                // the count keeps growing past the cap, so readers must clamp to maxLightsPerCluster
                let slot = atomicAdd(&clusterCounts[index], 1u);
                if (slot < maxLightsPerCluster) {
                    clusterLightIndices[index * maxLightsPerCluster + slot] = lightIdx;
                }
            }
        }
    }
}
