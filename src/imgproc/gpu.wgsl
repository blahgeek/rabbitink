struct Params {
  width: u32,
  height: u32,
  rgba_pitch: u32,
  bw_pitch: u32,
}

@group(0) @binding(0)
var<uniform> params: Params;

// WGSL does only have u32/i32, no u8

@group(0) @binding(1)
var<storage, read> rgba_img: array<u32>;

// the BW image is packed as 1 BIT per pixel

@group(0) @binding(2)
var<storage, read_write> bw_img: array<u32>;


var<workgroup> bw_32bit_arr: array<atomic<u32>, 2>;  // must match the workgroup size


@compute
@workgroup_size(64,1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>,
        @builtin(local_invocation_id) local_id: vec3<u32>) {
  let x = global_id.x;
  let y = global_id.y;

  let workgroup_bw_32bit = &bw_32bit_arr[local_id.x / 32u];  // must match the workgroup size

  if (x < params.width && y < params.height) {
    let red: u32 = rgba_img[y * params.rgba_pitch / 4u + x] & 0xffu;
    if ((red & 0x80u) != 0u) {
      atomicOr(workgroup_bw_32bit, 1u << (x % 32u));
    }
  }

  workgroupBarrier();

  if (x % 32u == 0u && y < params.height) {
    bw_img[y * params.bw_pitch / 4u + x / 32u] = *workgroup_bw_32bit;
  }
}