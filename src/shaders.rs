// CHECKITOUT: this file loads all the shaders and preprocesses them with some common code

use std::sync::LazyLock;

// CONSTANTS (for use in shaders)
// =================================

// CHECKITOUT: feel free to add more constants here and to refer to them in your shader code.
// A new constant needs two lines: the `pub const` below for Rust code, and an entry in
// `shader_constants()` so the preprocessor can replace `${name}` in WGSL.

pub const BIND_GROUP_SCENE: u32 = 0;
pub const BIND_GROUP_MODEL: u32 = 1;
pub const BIND_GROUP_MATERIAL: u32 = 2;

pub const MOVE_LIGHTS_WORKGROUP_SIZE: u32 = 128;

pub const LIGHT_RADIUS: f32 = 2.0;

/// The names shaders use for the constants above, as in `@group(${bindGroup_scene})`.
fn shader_constants() -> Vec<(&'static str, String)> {
    vec![
        ("bindGroup_scene", BIND_GROUP_SCENE.to_string()),
        ("bindGroup_model", BIND_GROUP_MODEL.to_string()),
        ("bindGroup_material", BIND_GROUP_MATERIAL.to_string()),
        (
            "moveLightsWorkgroupSize",
            MOVE_LIGHTS_WORKGROUP_SIZE.to_string(),
        ),
        // f32 formats without a trailing ".0" when it's a whole number ("2"), which WGSL
        // reads as an abstract number, the same as the TypeScript version produced.
        ("lightRadius", LIGHT_RADIUS.to_string()),
    ]
}

// =================================

/// Replaces every `${name}` in `raw` with the value of the constant called `name`.
fn eval_shader_raw(raw: &str) -> String {
    let constants = shader_constants();

    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let end = after
            .find('}')
            .unwrap_or_else(|| panic!("unclosed `${{` in shader source: {}", &rest[start..]));
        let name = after[..end].trim();
        let value = constants
            .iter()
            .find(|(constant_name, _)| *constant_name == name)
            .map(|(_, value)| value)
            .unwrap_or_else(|| {
                panic!("unknown shader constant `${{{name}}}`; add it to shader_constants() in src/shaders.rs")
            });
        out.push_str(value);
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

/// Prepends common.wgsl to a shader and fills in its constants.
fn process_shader_raw(raw: &str) -> String {
    let common_src = eval_shader_raw(include_str!("../shaders/common.wgsl"));
    common_src + &eval_shader_raw(raw)
}

// `include_str!` embeds each file into the executable at compile time, so shader edits take
// effect on the next `cargo run` (which rebuilds), not while the program is running.
// `LazyLock` runs the preprocessing the first time a shader is used, then keeps the result.
//
// `#[expect(dead_code)]` silences the "never used" warning for shaders whose renderer you
// haven't written yet. Once you use one, the compiler warns that the expectation is
// unfulfilled; delete the attribute then.

pub static NAIVE_VERT_SRC: LazyLock<String> =
    LazyLock::new(|| process_shader_raw(include_str!("../shaders/naive.vs.wgsl")));
pub static NAIVE_FRAG_SRC: LazyLock<String> =
    LazyLock::new(|| process_shader_raw(include_str!("../shaders/naive.fs.wgsl")));

#[expect(dead_code, reason = "used by Forward+ (TODO-2)")]
pub static FORWARD_PLUS_FRAG_SRC: LazyLock<String> =
    LazyLock::new(|| process_shader_raw(include_str!("../shaders/forward_plus.fs.wgsl")));

#[expect(dead_code, reason = "used by Clustered Deferred (TODO-3)")]
pub static CLUSTERED_DEFERRED_FRAG_SRC: LazyLock<String> =
    LazyLock::new(|| process_shader_raw(include_str!("../shaders/clustered_deferred.fs.wgsl")));
#[expect(dead_code, reason = "used by Clustered Deferred (TODO-3)")]
pub static CLUSTERED_DEFERRED_FULLSCREEN_VERT_SRC: LazyLock<String> = LazyLock::new(|| {
    process_shader_raw(include_str!(
        "../shaders/clustered_deferred_fullscreen.vs.wgsl"
    ))
});
#[expect(dead_code, reason = "used by Clustered Deferred (TODO-3)")]
pub static CLUSTERED_DEFERRED_FULLSCREEN_FRAG_SRC: LazyLock<String> = LazyLock::new(|| {
    process_shader_raw(include_str!(
        "../shaders/clustered_deferred_fullscreen.fs.wgsl"
    ))
});

pub static MOVE_LIGHTS_COMPUTE_SRC: LazyLock<String> =
    LazyLock::new(|| process_shader_raw(include_str!("../shaders/move_lights.cs.wgsl")));
#[expect(dead_code, reason = "used by light clustering (TODO-2)")]
pub static CLUSTERING_COMPUTE_SRC: LazyLock<String> =
    LazyLock::new(|| process_shader_raw(include_str!("../shaders/clustering.cs.wgsl")));
