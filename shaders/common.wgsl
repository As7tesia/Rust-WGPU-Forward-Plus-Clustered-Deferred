// CHECKITOUT: code that you add here will be prepended to all shaders

struct Light {
    pos: vec3f,
    color: vec3f
}

struct LightSet {
    numLights: u32,
    lights: array<Light>
}

// Typed copies of the shaders.rs constants, so shader code can use them by name.
const lightRadius: f32 = ${lightRadius};
const gridRes = vec3u(${gridResX}, ${gridResY}, ${gridResZ});
const maxLightsPerCluster: u32 = ${maxLightsPerCluster};
const clusterNear: f32 = ${clusterNear};
const clusterFar: f32 = ${clusterFar};

// Z slice containing a view-space depth
fn sliceFromDepth(depth: f32) -> u32 {
        
    let d = max(depth, clusterNear);
    let k = floor(log(d / clusterNear) / log(clusterFar / clusterNear) * f32(gridRes.z));

    return u32(min(k, f32(gridRes.z - 1u)));
}

fn clusterIndex(x: u32, y: u32, z: u32) -> u32 {
    return x + y * gridRes.x + z * gridRes.x * gridRes.y;
}

// NDC y points up, so grid row 0 is the bottom of the screen.
fn gridXYFromNdc(ndc: vec2f) -> vec2u {
    let gridResF = vec2f(gridRes.xy);
    return vec2u(clamp(floor((ndc * 0.5 + 0.5) * gridResF), vec2f(0.0), gridResF - 1.0));
}

// For points in front of the camera, i.e. visible fragments.
fn clusterCoordsAt(posWorld: vec3f, viewProj: mat4x4f, view: mat4x4f) -> vec3u {
    let clip = viewProj * vec4f(posWorld, 1.0);
    let xy = gridXYFromNdc(clip.xy / clip.w);
    let depth = -(view * vec4f(posWorld, 1.0)).z;
    return vec3u(xy, sliceFromDepth(depth));
}

struct CameraUniforms {
    viewProj: mat4x4f,
    view: mat4x4f,
    invProj: mat4x4f,
    proj: mat4x4f,
}

// CHECKITOUT: this special attenuation function ensures lights don't affect geometry outside the maximum light radius
fn rangeAttenuation(distance: f32) -> f32 {
    return clamp(1.f - pow(distance / ${lightRadius}, 4.f), 0.f, 1.f) / (distance * distance);
}

fn calculateLightContrib(light: Light, posWorld: vec3f, nor: vec3f) -> vec3f {
    let vecToLight = light.pos - posWorld;
    let distToLight = length(vecToLight);

    let lambert = max(dot(nor, normalize(vecToLight)), 0.f);
    return light.color * lambert * rangeAttenuation(distToLight);
}
