// One triangle with corners (-1, -1), (3, -1), (-1, 3) covers the whole screen, with no vertex
// buffer. Draw it with 3 vertices.
@vertex
fn main(@builtin(vertex_index) vertexIndex: u32) -> @builtin(position) vec4f
{
    let corner = vec2f(f32((vertexIndex << 1u) & 2u), f32(vertexIndex & 2u));
    return vec4f(corner * 2.0 - 1.0, 0.0, 1.0);
}
