use log::debug;
use wgpu::util::DeviceExt;

use super::{DitheringMethod, MonoImgprocOptions};
use crate::image::*;

pub struct MonoImgproc {
    opts: MonoImgprocOptions,

    device: wgpu::Device,
    queue: wgpu::Queue,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::ComputePipeline,

    input_buffer: wgpu::Buffer,
    input_stage_buffer: wgpu::Buffer,
    output_buffer: wgpu::Buffer,
    output_stage_buffer: wgpu::Buffer,

    dithering_threshold_buffer: wgpu::Buffer,
    current_dithering_method: DitheringMethod,
}

const WORKGROUP_SIZE: (i32, i32) = (64, 1);

const BAYERS4_THRESHOLDS: [u32; 16] = [
    0, 128, 32, 160, 192, 64, 224, 96, 48, 176, 16, 144, 240, 112, 208, 80,
];

const BAYERS2_THRESHOLDS: [u32; 16] = [
    0, 128, 0, 128, 192, 64, 192, 64, 0, 128, 0, 128, 192, 64, 192, 64,
];

const NO_DITHERING_THRESHOLDS: [u32; 16] = [128; 16];

fn dithering_thresholds_buf(v: &[u32; 16]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, 16 * 4) }
}

impl MonoImgproc {
    pub async fn new_async(opts: MonoImgprocOptions) -> Self {
        assert!(
            opts.input_pitch % 4 == 0 && opts.output_pitch % 4 == 0,
            "gpu imgproc requires 4byte aligned"
        );

        let instance = wgpu::Instance::default();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .unwrap();
        debug!("Initializing GPU imgproc: {:?}", adapter.get_info());

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default(), None)
            .await
            .unwrap();

        let input_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bgra"),
            size: (opts.image_size.height * opts.input_pitch) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let input_stage_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("input_staging"),
            size: (opts.image_size.height * opts.input_pitch) as u64,
            usage: wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::MAP_WRITE,
            mapped_at_creation: false,
        });

        let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bw"),
            size: (opts.image_size.height * opts.output_pitch) as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let output_stage_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("output_staging"),
            size: (opts.image_size.height * opts.output_pitch) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let params_data: Vec<i32> = vec![
            opts.image_size.width,
            opts.image_size.height,
            opts.input_pitch,
            opts.output_pitch,
        ];
        let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("params"),
            contents: unsafe { std::slice::from_raw_parts(params_data.as_ptr() as *const u8, 16) },
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let dithering_threshold_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("bayers4"),
                contents: dithering_thresholds_buf(&BAYERS4_THRESHOLDS),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            });

        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!("gpu.wgsl"))),
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: None,
            module: &shader_module,
            entry_point: "main",
        });
        let bind_group_layout = pipeline.get_bind_group_layout(0);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: input_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: output_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: dithering_threshold_buffer.as_entire_binding(),
                },
            ],
        });

        Self {
            opts,
            device,
            queue,
            bind_group,
            pipeline,
            input_buffer,
            input_stage_buffer,
            output_buffer,
            output_stage_buffer,
            dithering_threshold_buffer,
            current_dithering_method: DitheringMethod::Bayers4,
        }
    }

    fn map_buffer_sync(&self, buffer_slice: &wgpu::BufferSlice, mode: wgpu::MapMode) {
        let (sender, receiver) = std::sync::mpsc::channel::<()>();
        buffer_slice.map_async(mode, move |v| {
            v.expect("failed to map buffer");
            sender.send(()).unwrap();
        });
        self.device.poll(wgpu::Maintain::Wait);
        receiver.recv().unwrap();
    }

    fn write_input(&self, input_img: &impl ConstImage<32>) {
        let slice = self.input_stage_buffer.slice(..);
        self.map_buffer_sync(&slice, wgpu::MapMode::Write);
        let mut stage_buf = slice.get_mapped_range_mut();
        let mut stage_buf_img = ImageView::<32>::new(
            &mut stage_buf,
            self.opts.image_size.width,
            self.opts.image_size.height,
            Some(self.opts.input_pitch),
        );
        stage_buf_img.copy_from(input_img);
        drop(stage_buf);
        self.input_stage_buffer.unmap();
    }

    fn read_output(&self, output_img: &mut impl Image<1>) {
        let slice = self.output_stage_buffer.slice(..);
        self.map_buffer_sync(&slice, wgpu::MapMode::Read);
        let output_buf = slice.get_mapped_range();
        let output_buf_img = ConstImageView::<1>::new(
            &output_buf,
            self.opts.image_size.width,
            self.opts.image_size.height,
            Some(self.opts.output_pitch),
        );
        output_img.copy_from(&output_buf_img);
        drop(output_buf);
        self.output_stage_buffer.unmap();
    }

    pub fn new(options: MonoImgprocOptions) -> Self {
        pollster::block_on(Self::new_async(options))
    }

    pub fn process(
        &mut self,
        input_img: &impl ConstImage<32>,
        output_img: &mut impl Image<1>,
        dithering_method: DitheringMethod,
    ) {
        let t_start = std::time::Instant::now();

        self.write_input(input_img);
        if dithering_method != self.current_dithering_method {
            self.queue.write_buffer(
                &self.dithering_threshold_buffer,
                0,
                dithering_thresholds_buf(match dithering_method {
                    DitheringMethod::Bayers2 => &BAYERS2_THRESHOLDS,
                    DitheringMethod::Bayers4 => &BAYERS4_THRESHOLDS,
                    DitheringMethod::NoDithering => &NO_DITHERING_THRESHOLDS,
                }),
            );
            self.current_dithering_method = dithering_method;
        }

        let t_uploaded = std::time::Instant::now();

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        encoder.copy_buffer_to_buffer(
            &self.input_stage_buffer,
            0,
            &self.input_buffer,
            0,
            self.input_buffer.size(),
        );
        encoder.clear_buffer(
            &self.output_buffer,
            0,
            wgpu::BufferSize::new(self.output_buffer.size()),
        );

        {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
            cpass.set_pipeline(&self.pipeline);
            cpass.set_bind_group(0, &self.bind_group, &[]);
            cpass.dispatch_workgroups(
                (self.opts.image_size.width as f32 / WORKGROUP_SIZE.0 as f32).ceil() as u32,
                (self.opts.image_size.height as f32 / WORKGROUP_SIZE.1 as f32).ceil() as u32,
                1,
            );
        }

        encoder.copy_buffer_to_buffer(
            &self.output_buffer,
            0,
            &self.output_stage_buffer,
            0,
            self.output_buffer.size(),
        );

        self.queue.submit(Some(encoder.finish()));
        self.device.poll(wgpu::Maintain::Wait);
        let t_computed = std::time::Instant::now();

        self.read_output(output_img);
        let t_downloaded = std::time::Instant::now();

        debug!(
            "GPU imgproc processed one frame {:?}: upload {:?}, compute {:?}, download {:?}",
            self.opts.image_size,
            t_uploaded - t_start,
            t_computed - t_uploaded,
            t_downloaded - t_computed
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic() {
        let input_img_data = {
            let mut v = Vec::<u8>::new();
            for i in 0..(4 * 32) {
                v.push(if (i / 4) % 2 == 0 { 0xff } else { 0 });
            }
            v
        };
        let color_img = ConstImageView::<32>::new(input_img_data.as_slice(), 32, 1, None);

        let mut output_img_data: Vec<u8> = vec![0; 4];
        let mut output_img = ImageView::<1>::new(output_img_data.as_mut_slice(), 32, 1, None);

        let mut imgproc = MonoImgproc::new(MonoImgprocOptions {
            image_size: color_img.size(),
            input_pitch: color_img.pitch(),
            output_pitch: output_img.pitch(),
        });
        imgproc.process(&color_img, &mut output_img, DitheringMethod::Bayers4);

        drop(output_img);

        assert_eq!(output_img_data[0], 0b01010101);
        assert_eq!(output_img_data[1], 0b01010101);
        assert_eq!(output_img_data[2], 0b01010101);
        assert_eq!(output_img_data[3], 0b01010101);
    }
}
