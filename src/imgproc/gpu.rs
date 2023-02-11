use wgpu::util::DeviceExt;
use log::trace;

use crate::image::*;

#[derive(Clone, Copy, Debug)]
pub struct ImgprocOptions {
    pub image_size: Size,
    pub rgba_pitch: i32,
    pub bw_pitch: i32,
}

pub struct GpuImgproc {
    opts: ImgprocOptions,

    device: wgpu::Device,
    queue: wgpu::Queue,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::ComputePipeline,

    rgba_buffer: wgpu::Buffer,
    rgba_stage_buffer: wgpu::Buffer,
    bw_buffer: wgpu::Buffer,
    bw_stage_buffer: wgpu::Buffer,
}

const WORKGROUP_SIZE: (i32, i32) = (32, 2);

impl GpuImgproc {
    pub async fn new(opts: ImgprocOptions) -> GpuImgproc {
        assert!(opts.rgba_pitch % 4 == 0 && opts.bw_pitch % 4 == 0,
                "gpu imgproc requires 4byte aligned");

        let instance = wgpu::Instance::default();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .unwrap();
        trace!("Initializing GPU imgproc: {:?}", adapter.get_info());

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default(), None)
            .await
            .unwrap();

        let rgba_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rgba"),
            size: (opts.image_size.height * opts.rgba_pitch) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let rgba_stage_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rgba_staging"),
            size: (opts.image_size.height * opts.rgba_pitch) as u64,
            usage: wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::MAP_WRITE,
            mapped_at_creation: false,
        });

        let bw_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bw"),
            size: (opts.image_size.height * opts.bw_pitch) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bw_stage_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bw_staging"),
            size: (opts.image_size.height * opts.bw_pitch) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let params_data: Vec<i32> = vec![
            opts.image_size.width,
            opts.image_size.height,
            opts.rgba_pitch,
            opts.bw_pitch,
        ];
        let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("params"),
            contents: unsafe { std::slice::from_raw_parts(params_data.as_ptr() as *const u8, 16) },
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
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
                    resource: rgba_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: bw_buffer.as_entire_binding(),
                },
            ],
        });

        GpuImgproc {
            opts,
            device,
            queue,
            bind_group,
            pipeline,
            rgba_buffer,
            rgba_stage_buffer,
            bw_buffer,
            bw_stage_buffer,
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

    fn write_input(&self, rgba_img: &impl ConstImage<32>) {
        let slice = self.rgba_stage_buffer.slice(..);
        self.map_buffer_sync(&slice, wgpu::MapMode::Write);
        let mut stage_buf = slice.get_mapped_range_mut();
        let mut stage_buf_img = ImageView::<32>::new(
            &mut stage_buf,
            self.opts.image_size.width,
            self.opts.image_size.height,
            Some(self.opts.rgba_pitch),
        );
        stage_buf_img.copy_from(rgba_img);
        drop(stage_buf);
        self.rgba_stage_buffer.unmap();
    }

    fn read_output(&self, bw_img: &mut impl Image<1>) {
        let slice = self.bw_stage_buffer.slice(..);
        self.map_buffer_sync(&slice, wgpu::MapMode::Read);
        let bw_buf = slice.get_mapped_range();
        let bw_buf_img = ConstImageView::<1>::new(
            &bw_buf,
            self.opts.image_size.width,
            self.opts.image_size.height,
            Some(self.opts.bw_pitch),
        );
        bw_img.copy_from(&bw_buf_img);
        drop(bw_buf);
        self.bw_stage_buffer.unmap();
    }

    pub fn process(&self, input_rgba_img: &impl ConstImage<32>, output_bw_img: &mut impl Image<1>) {
        let t_start = std::time::Instant::now();

        self.write_input(input_rgba_img);
        let t_uploaded = std::time::Instant::now();

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        encoder.copy_buffer_to_buffer(
            &self.rgba_stage_buffer,
            0,
            &self.rgba_buffer,
            0,
            self.rgba_buffer.size(),
        );
        encoder.clear_buffer(&self.bw_buffer, 0, wgpu::BufferSize::new(self.bw_buffer.size()));

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
            &self.bw_buffer,
            0,
            &self.bw_stage_buffer,
            0,
            self.bw_buffer.size(),
        );

        self.queue.submit(Some(encoder.finish()));
        self.device.poll(wgpu::Maintain::Wait);
        let t_computed = std::time::Instant::now();

        self.read_output(output_bw_img);
        let t_downloaded = std::time::Instant::now();

        trace!("GPU imgproc processed one frame: upload {:?}, compute {:?}, download {:?}",
               t_uploaded - t_start, t_computed - t_uploaded, t_downloaded - t_computed);
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic() {
        let rgba_img_data = {
            let mut v = Vec::<u8>::new();
            for i in 0..(4 * 32) {
                v.push(if (i / 4) % 2 == 0 { 0xff } else { 0 });
            }
            v
        };
        let rgb_img = ConstImageView::<32>::new(rgba_img_data.as_slice(), 32, 1, None);

        let mut bw_img_data: Vec<u8> = vec![0; 4];
        let mut bw_img = ImageView::<1>::new(bw_img_data.as_mut_slice(), 32, 1, None);

        let imgproc = GpuImgproc::new(ImgprocOptions {
            image_size: rgb_img.size(),
            rgba_pitch: rgb_img.pitch(),
            bw_pitch: bw_img.pitch(),
        });
        let imgproc = pollster::block_on(imgproc);
        imgproc.process(&rgb_img, &mut bw_img);

        drop(bw_img);

        assert_eq!(bw_img_data[0], 0b01010101);
        assert_eq!(bw_img_data[1], 0b01010101);
        assert_eq!(bw_img_data[2], 0b01010101);
        assert_eq!(bw_img_data[3], 0b01010101);
    }
}