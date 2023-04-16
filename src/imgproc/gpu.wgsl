struct Params {
  input_width: u32,
  input_height: u32,
  input_pitch: u32,
  output_width: u32,
  output_height: u32,
  output_pitch: u32,
}

@group(0) @binding(0)
var<uniform> params: Params;

// WGSL does only have u32/i32, no u8

@group(0) @binding(1)
var<storage, read> input_img: array<u32>;

// the BW image is packed as 1 BIT per pixel

@group(0) @binding(2)
var<storage, read_write> output_img: array<u32>;

@group(0) @binding(3)
var<storage, read> thresholds4x4: array<u32>;

@group(0) @binding(4)
var<uniform> coord_transform: mat3x2<f32>;


fn rgb_to_gray_with_dithering(rgb: vec3<u32>, x: u32, y: u32) -> u32 {
  let gray = 0.3 * f32(rgb.x) + 0.59 * f32(rgb.y) + 0.11 * f32(rgb.z); // Luminosity Method
  // let gray = (rgb.x + rgb.y + rgb.z) / 3u;
  let threshold = thresholds4x4[(y % 4u) * 4u + (x % 4u)];
  if (u32(gray) <= threshold) {
    return 0u;
  } else {
    return 1u;
  }
}


var<workgroup> output_32bit_arr: array<atomic<u32>, 2>;  // must match the workgroup size

@compute
@workgroup_size(64,1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>,
        @builtin(local_invocation_id) local_id: vec3<u32>) {
  let x = global_id.x;
  let y = global_id.y;

  let workgroup_output_32bit = &output_32bit_arr[local_id.x / 32u];

  if (x < params.output_width && y < params.output_height) {
    let input_coord: vec2<f32> = coord_transform * vec3(f32(x) + 0.5, f32(y) + 0.5, 1.0);
    let input_coord_x: u32 = u32(clamp(input_coord.x, 0.0, f32(params.input_width) - 1.0));
    let input_coord_y: u32 = u32(clamp(input_coord.y, 0.0, f32(params.input_height) - 1.0));

    let bgra: u32 = input_img[input_coord_y * params.input_pitch / 4u + input_coord_x];
    // little endian
    let gray = rgb_to_gray_with_dithering(
      vec3((bgra >> 16u) & 0xffu, (bgra >> 8u) & 0xffu, bgra & 0xffu), x, y);

    if (gray != 0u) {
      atomicOr(workgroup_output_32bit, 1u << (x % 32u));
    }
  }

  workgroupBarrier();

  if (x % 32u == 0u && y < params.output_height) {
    output_img[y * params.output_pitch / 4u + x / 32u] = *workgroup_output_32bit;
  }
}