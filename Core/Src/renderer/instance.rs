use cgmath::{prelude::*, Vector3, Matrix4, Quaternion};

// Logiczna reprezentacja bloku w świecie
pub struct Instance {
    pub position: Vector3<f32>,
    pub rotation: Quaternion<f32>,
}

impl Instance {
    // Konwersja na format zrozumiały dla Shadera (Macierz Modelu)
    pub fn to_raw(&self) -> InstanceRaw {
        InstanceRaw {
            model_matrix: (Matrix4::from_translation(self.position) * Matrix4::from(self.rotation)).into(),
        }
    }
}

// Struktura mapowana bezpośrednio na bufor GPU (InstanceInput w shaderze)
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct InstanceRaw {
    model_matrix: [[f32; 4]; 4],
}

impl InstanceRaw {
    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<InstanceRaw>() as wgpu::BufferAddress,
            // To mówi GPU: zmieniaj te dane co instancję bloku, nie co wierzchołek!
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                // Macierz 4x4 zajmuje 4 sloty lokalizacji (vec4 każdy)
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 7,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 12]>() as wgpu::BufferAddress,
                    shader_location: 8,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

// Funkcja pomocnicza do budowania świata (wywołaj to w mod.rs)
pub fn create_world_map(size: i32) -> Vec<Instance> {
    use cgmath::{Deg, Rotation3};
    let mut instances = Vec::new();

    for z in 0..size {
        for x in 0..size {
            let x_pos = (x - size / 2) as f32 * 1.05;
            let z_pos = (z - size / 2) as f32 * 1.05;
            
            // Lekka fala sinusa, żeby świat nie był płaski jak decha
            let y_pos = f32::sin(x as f32 * 0.5) * 0.3;

            let position = Vector3 { x: x_pos, y: y_pos, z: z_pos };
            let rotation = Quaternion::from_axis_angle(Vector3::unit_y(), Deg(0.0));

            instances.push(Instance { position, rotation });
        }
    }
    instances
}