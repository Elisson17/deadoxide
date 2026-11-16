//! zombie.core.skinnedmodel.model.VertexBufferObject: the CPU-side vertex formats and arrays (the GL buffer upload
//! is not ported).

/// VertexBufferObject.VertexType
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VertexType {
    VertexArray,
    NormalArray,
    ColorArray,
    IndexArray,
    TextureCoordArray,
    TangentArray,
    BlendWeightArray,
    BlendIndexArray,
    Depth,
}

impl VertexType {
    /// Enum.name()
    pub fn name(self) -> &'static str {
        match self {
            VertexType::VertexArray => "VertexArray",
            VertexType::NormalArray => "NormalArray",
            VertexType::ColorArray => "ColorArray",
            VertexType::IndexArray => "IndexArray",
            VertexType::TextureCoordArray => "TextureCoordArray",
            VertexType::TangentArray => "TangentArray",
            VertexType::BlendWeightArray => "BlendWeightArray",
            VertexType::BlendIndexArray => "BlendIndexArray",
            VertexType::Depth => "Depth",
        }
    }
}

/// VertexBufferObject.VertexElement
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VertexElement {
    pub element_type: Option<VertexType>,
    pub byte_size: i32,
    pub byte_offset: i32,
}

/// VertexBufferObject.VertexFormat
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VertexFormat {
    pub elements: Vec<VertexElement>,
    pub stride: i32,
}

impl VertexFormat {
    /// VertexFormat(int numElements)
    pub fn new(num_elements: usize) -> Self {
        VertexFormat {
            elements: vec![
                VertexElement {
                    element_type: None,
                    byte_size: 0,
                    byte_offset: 0,
                };
                num_elements
            ],
            stride: 0,
        }
    }

    /// VertexFormat.setElement(int, VertexType, int)
    pub fn set_element(&mut self, index: usize, element_type: VertexType, byte_size: i32) {
        self.elements[index].element_type = Some(element_type);
        self.elements[index].byte_size = byte_size;
    }

    /// VertexFormat.indexOf(VertexType): -1 when absent.
    pub fn index_of(&self, vertex_type: VertexType) -> i32 {
        self.elements
            .iter()
            .position(|e| e.element_type == Some(vertex_type))
            .map_or(-1, |i| i as i32)
    }

    /// VertexFormat.calculate: the byte offset of each element and the stride.
    pub fn calculate(&mut self) {
        self.stride = 0;
        for e in &mut self.elements {
            e.byte_offset = self.stride;
            self.stride += e.byte_size;
        }
    }
}

/// VertexBufferObject.VertexArray: `numVertices * stride` bytes (the little-endian direct buffer of
/// `BufferUtils.createByteBuffer`), floats written and read by absolute byte index.
#[derive(Clone, Debug, PartialEq)]
pub struct VertexArray {
    pub format: VertexFormat,
    pub num_vertices: i32,
    pub buffer: Vec<u8>,
}

impl VertexArray {
    /// VertexArray(VertexFormat, int)
    pub fn new(format: &VertexFormat, num_vertices: i32) -> Result<Self, String> {
        let size = i64::from(num_vertices) * i64::from(format.stride);
        if !(0..=i64::from(i32::MAX)).contains(&size) {
            return Err("java.lang.IllegalArgumentException".to_string());
        }
        Ok(VertexArray {
            format: format.clone(),
            num_vertices,
            buffer: vec![0u8; size as usize],
        })
    }

    fn put_float(&mut self, index: i32, v: f32) {
        let i = index as usize;
        self.buffer[i..i + 4].copy_from_slice(&v.to_le_bytes());
    }

    fn base(&self, vertex: i32, element: usize) -> i32 {
        vertex
            .wrapping_mul(self.format.stride)
            .wrapping_add(self.format.elements[element].byte_offset)
    }

    /// VertexArray.setElement(vertex, element, v1, v2)
    pub fn set_element2(&mut self, vertex: i32, element: usize, v1: f32, v2: f32) {
        let i = self.base(vertex, element);
        self.put_float(i, v1);
        self.put_float(i + 4, v2);
    }

    /// VertexArray.setElement(vertex, element, v1, v2, v3)
    pub fn set_element3(&mut self, vertex: i32, element: usize, v1: f32, v2: f32, v3: f32) {
        let i = self.base(vertex, element);
        self.put_float(i, v1);
        self.put_float(i + 4, v2);
        self.put_float(i + 8, v3);
    }

    /// VertexArray.setElement(vertex, element, v1, v2, v3, v4)
    pub fn set_element4(
        &mut self,
        vertex: i32,
        element: usize,
        v1: f32,
        v2: f32,
        v3: f32,
        v4: f32,
    ) {
        let i = self.base(vertex, element);
        self.put_float(i, v1);
        self.put_float(i + 4, v2);
        self.put_float(i + 8, v3);
        self.put_float(i + 12, v4);
    }

    /// VertexArray.getElementFloat(vertex, element, n)
    pub fn get_element_float(&self, vertex: i32, element: usize, n: i32) -> f32 {
        let i = (self.base(vertex, element) + n * 4) as usize;
        f32::from_le_bytes([
            self.buffer[i],
            self.buffer[i + 1],
            self.buffer[i + 2],
            self.buffer[i + 3],
        ])
    }
}
