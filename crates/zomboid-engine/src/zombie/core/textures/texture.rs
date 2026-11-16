//! zombie.core.textures.Texture as a client without a renderer has it: the texture packs' index (which sub-texture
//! lives where, TexturePackDevice's metadata), the lookup GameWindow builds from it, and `Texture.getSharedTexture`'s
//! search (a pack sub-texture by its short name, else a file of the game folder). A texture never gets its pixels
//! here (they load on the render thread), so it is never ready: its size is the sub-texture's, or for a file the
//! size `PNGSize` reads from the file's header when first asked.
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::zombie::zomboid_file_system::ZomboidFileSystem;

/// zombie.core.textures.TexturePackPage.SubTextureInfo
#[derive(Clone, Debug)]
pub struct SubTextureInfo {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub ox: i32,
    pub oy: i32,
    pub fx: i32,
    pub fy: i32,
    pub name: String,
}

/// zombie.fileSystem.FileSystem.SubTexture: a sub-texture and the pack and page it is on.
#[derive(Clone, Debug)]
pub struct SubTexture {
    pub pack_name: String,
    pub page_name: String,
    pub info: SubTextureInfo,
}

/// The `.pack` reader of TexturePackDevice with TexturePackPage's static `readInt`/`readIntByte` registers
/// (`chl1`..`chl4`, the last four bytes read as an int).
struct PackReader<R: Read + Seek> {
    input: BufReader<R>,
    chl: [i32; 4],
}

impl<R: Read + Seek> PackReader<R> {
    /// InputStream.read: a byte, or -1 at the end.
    fn read(&mut self) -> Result<i32, String> {
        let mut b = [0u8; 1];
        match self.input.read(&mut b) {
            Ok(0) => Ok(-1),
            Ok(_) => Ok(i32::from(b[0])),
            Err(e) => Err(e.to_string()),
        }
    }

    /// zombie.core.textures.TexturePackPage.readInt: four bytes, little-endian; EOFException at the end.
    fn read_int(&mut self) -> Result<i32, String> {
        let ch1 = self.read()?;
        let ch2 = self.read()?;
        let ch3 = self.read()?;
        let ch4 = self.read()?;
        self.chl = [ch1, ch2, ch3, ch4];
        if (ch1 | ch2 | ch3 | ch4) < 0 {
            return Err("java.io.EOFException".into());
        }
        Ok(ch1
            .wrapping_add(ch2 << 8)
            .wrapping_add(ch3 << 16)
            .wrapping_add(ch4 << 24))
    }

    /// zombie.core.textures.TexturePackPage.readIntByte: the last three bytes read and one more.
    fn read_int_byte(&mut self) -> Result<i32, String> {
        let ch1 = self.chl[1];
        let ch2 = self.chl[2];
        let ch3 = self.chl[3];
        let ch4 = self.read()?;
        self.chl = [ch1, ch2, ch3, ch4];
        if (ch1 | ch2 | ch3 | ch4) < 0 {
            return Err("java.io.EOFException".into());
        }
        Ok(ch1
            .wrapping_add(ch2 << 8)
            .wrapping_add(ch3 << 16)
            .wrapping_add(ch4 << 24))
    }

    /// zombie.core.textures.TexturePackPage.ReadString: an int length, then one char per byte.
    fn read_string(&mut self) -> Result<String, String> {
        let size = self.read_int()?;
        let mut v = String::new();
        for _ in 0..size {
            // (char) input.read(): -1 at the end becomes U+FFFF
            let c = self.read()?;
            v.push(char::from_u32(c as u16 as u32).unwrap_or('\u{fffd}'));
        }
        Ok(v)
    }
}

/// zombie.fileSystem.TexturePackDevice.initMetaData then getSubTextureInfo: the sub-textures of the pack file
/// `filename`, by name (a later page's sub-texture of the same name replaces an earlier one, as in `submap`). An
/// IOException (a missing file, a bad version) is `Err`, which the original logs, leaving the pack empty.
pub fn texture_pack_sub_textures(
    pack_name: &str,
    filename: &Path,
) -> Result<HashMap<String, SubTexture>, String> {
    let file = std::fs::File::open(filename).map_err(|e| e.to_string())?;
    let mut input = PackReader {
        input: BufReader::new(file),
        chl: [0; 4],
    };
    let mut m = [0i32; 4];
    for b in &mut m {
        *b = input.read()?;
    }
    let version = if m == [80, 90, 80, 75] {
        let version = input.read_int()?;
        if !(1..=1).contains(&version) {
            return Err(format!("invalid .pack file version {version}"));
        }
        version
    } else {
        // input.reset() to the mark at 0
        input
            .input
            .seek(SeekFrom::Start(0))
            .map_err(|e| e.to_string())?;
        0
    };
    let mut submap: HashMap<String, SubTexture> = HashMap::new();
    let count = input.read_int()?;
    for _ in 0..count {
        // readPage
        let page_name = input.read_string()?;
        let num_entries = input.read_int()?;
        let _has_alpha = input.read_int()? != 0;
        let mut sub = Vec::new();
        for _ in 0..num_entries {
            let name = input.read_string()?;
            let a = input.read_int()?;
            let b = input.read_int()?;
            let c = input.read_int()?;
            let d = input.read_int()?;
            let e = input.read_int()?;
            let f = input.read_int()?;
            let g = input.read_int()?;
            let h = input.read_int()?;
            sub.push(SubTextureInfo {
                x: a,
                y: b,
                w: c,
                h: d,
                ox: e,
                oy: f,
                fx: g,
                fy: h,
                name,
            });
        }
        if version == 0 {
            while input.read_int_byte()? != -559038737 {}
        } else {
            let id = input.read_int()?;
            input
                .input
                .seek_relative(i64::from(id))
                .map_err(|e| e.to_string())?;
        }
        for info in sub {
            submap.insert(
                info.name.clone(),
                SubTexture {
                    pack_name: pack_name.to_string(),
                    page_name: page_name.clone(),
                    info,
                },
            );
        }
    }
    Ok(submap)
}

/// zombie.core.textures.Texture, the fields a texture without pixels has.
#[derive(Debug)]
pub struct Texture {
    /// Texture.name: the sub-texture's name, or the file's path
    pub name: String,
    pub width: i32,
    pub height: i32,
    pub split_x: i32,
    pub split_y: i32,
    pub split_w: i32,
    pub split_h: i32,
    pub offset_x: i32,
    pub offset_y: i32,
    pub width_orig: i32,
    pub height_orig: i32,
    /// The path of its TextureID (`dataid.getPath()`): "@pack@/<pack>/<page>", or the file
    pub data_path: String,
}

pub type TextureRef = Rc<RefCell<Texture>>;

/// zombie.core.textures.PNGSize (the thread's one, `Texture.pngSize`): the size of the last header read; a failed
/// read leaves it as it was.
#[derive(Default, Debug)]
pub struct PngSize {
    pub width: i32,
    pub height: i32,
}

impl PngSize {
    /// zombie.core.textures.PNGSize.readSize(String): the signature, then the IHDR chunk's width and height. The
    /// original then checks the chunk's CRC; a mismatch throws after the size is set, so it is not checked here.
    pub fn read_size(&mut self, path: &Path) {
        let Ok(mut f) = std::fs::File::open(path) else {
            return;
        };
        let mut b = [0u8; 8 + 8 + 13];
        if f.read_exact(&mut b).is_err() {
            return;
        }
        if b[..8] != [0x89, b'P', b'N', b'G', 13, 10, 26, 10] || &b[12..16] != b"IHDR" {
            return;
        }
        let len = i32::from_be_bytes([b[8], b[9], b[10], b[11]]);
        if len != 13 {
            return;
        }
        self.width = i32::from_be_bytes([b[16], b[17], b[18], b[19]]);
        self.height = i32::from_be_bytes([b[20], b[21], b[22], b[23]]);
    }
}

impl Texture {
    /// zombie.core.textures.Texture.<init>(AssetPath, AssetManager, TextureAssetParams) for a pack sub-texture.
    fn from_sub_texture(sub: &SubTexture) -> Texture {
        Texture {
            name: sub.info.name.clone(),
            width: sub.info.w,
            height: sub.info.h,
            split_x: sub.info.x,
            split_y: sub.info.y,
            split_w: sub.info.w,
            split_h: sub.info.h,
            offset_x: sub.info.ox,
            offset_y: sub.info.oy,
            width_orig: sub.info.fx,
            height_orig: sub.info.fy,
            data_path: format!("@pack@/{}/{}", sub.pack_name, sub.page_name),
        }
    }

    /// The same constructor for a file: no size until it is read.
    fn from_file(path: &str) -> Texture {
        Texture {
            name: path.to_string(),
            width: 0,
            height: 0,
            split_x: -1,
            split_y: -1,
            split_w: 0,
            split_h: 0,
            offset_x: 0,
            offset_y: 0,
            width_orig: 0,
            height_orig: 0,
            data_path: path.to_string(),
        }
    }

    /// zombie.core.textures.Texture.syncReadSize: PNGSize.readSize of the TextureID's path, relative to the game
    /// folder.
    fn sync_read_size(&mut self, root: &Path, png: &mut PngSize) {
        png.read_size(&root.join(&self.data_path));
        self.width = png.width;
        self.height = png.height;
    }

    /// zombie.core.textures.Texture.getWidth: never ready here, so a width not set yet is read from the file.
    pub fn get_width(&mut self, root: &Path, png: &mut PngSize) -> i32 {
        if self.width <= 0 {
            self.sync_read_size(root, png);
        }
        self.width
    }

    /// zombie.core.textures.Texture.getHeight
    pub fn get_height(&mut self, root: &Path, png: &mut PngSize) -> i32 {
        if self.height <= 0 {
            self.sync_read_size(root, png);
        }
        self.height
    }

    /// zombie.core.textures.Texture.getWidthOrig
    pub fn get_width_orig(&mut self, root: &Path, png: &mut PngSize) -> i32 {
        if self.width_orig == 0 {
            self.get_width(root, png)
        } else {
            self.width_orig
        }
    }

    /// zombie.core.textures.Texture.getHeightOrig
    pub fn get_height_orig(&mut self, root: &Path, png: &mut PngSize) -> i32 {
        if self.height_orig == 0 {
            self.get_height(root, png)
        } else {
            self.height_orig
        }
    }
}

/// A pack GameWindow.LoadTexturePack mounted (GameWindow.TexturePack, no mod).
struct TexturePack {
    textures: HashMap<String, SubTexture>,
}

/// Texture's and GameWindow's static texture state.
pub struct Textures {
    /// The game folder (the process's working directory in the original).
    root: PathBuf,
    /// GameWindow.texturePacks
    texture_packs: Vec<TexturePack>,
    /// GameWindow.texturePackTextures
    texture_pack_textures: HashMap<String, SubTexture>,
    /// Texture.s_sharedTextureTable
    shared: HashMap<String, TextureRef>,
    /// Texture.nullTextures
    null_textures: HashSet<String>,
    /// TextureAssetManager's loaded assets, by asset path (one Texture per path)
    assets: HashMap<String, TextureRef>,
    /// BucketManager.Shared()'s textures
    bucket: HashMap<String, TextureRef>,
    /// Texture.pngSize
    pub png_size: PngSize,
}

impl Textures {
    pub fn new(root: &Path) -> Textures {
        Textures {
            root: root.to_path_buf(),
            texture_packs: Vec::new(),
            texture_pack_textures: HashMap::new(),
            shared: HashMap::new(),
            null_textures: HashSet::new(),
            assets: HashMap::new(),
            bucket: HashMap::new(),
            png_size: PngSize::default(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// zombie.GameWindow.LoadTexturePack(String, int): FileSystem.mountTexturePack of `media/texturepacks/<pack>.pack`
    /// (its index; a pack that fails to read is logged and stays empty).
    pub fn load_texture_pack(&mut self, fs: &ZomboidFileSystem, pack: &str) {
        let path = format!("media/texturepacks/{pack}.pack");
        let file_name = fs.get_string_or(&path).to_string();
        let textures =
            texture_pack_sub_textures(pack, &self.root.join(file_name)).unwrap_or_default();
        self.texture_packs.push(TexturePack { textures });
    }

    /// zombie.GameWindow.setTexturePackLookup: every pack's sub-textures, the first pack loaded winning a name; then
    /// Texture.onTexturePacksChanged (the shared table and the null names forgotten).
    pub fn set_texture_pack_lookup(&mut self) {
        self.texture_pack_textures.clear();
        for p in self.texture_packs.iter().rev() {
            for (k, v) in &p.textures {
                self.texture_pack_textures.insert(k.clone(), v.clone());
            }
        }
        self.null_textures.clear();
        self.shared.clear();
    }

    /// TextureAssetManager.load of a pack sub-texture (`@pack/<pack>/<page>/<name>`).
    fn load_sub_texture(&mut self, sub: &SubTexture) -> TextureRef {
        let path = format!(
            "@pack/{}/{}/{}",
            sub.pack_name, sub.page_name, sub.info.name
        );
        self.assets
            .entry(path)
            .or_insert_with(|| Rc::new(RefCell::new(Texture::from_sub_texture(sub))))
            .clone()
    }

    /// zombie.core.textures.Texture.getSharedTexture(String) (getSharedTextureInternal): cached by name; a ".png"
    /// name (not under "/mods/", not ".txt") by its file name without the extension in the packs' lookup; then the
    /// name itself in the lookup; then the bucket; a name without '.' is null; else the file of the game folder
    /// (looked up case-insensitively), null when there is none. A null name is the NullPointerException the original
    /// catches: null.
    pub fn get_shared_texture(
        &mut self,
        fs: &ZomboidFileSystem,
        name: Option<&str>,
    ) -> Option<TextureRef> {
        let name = name?;
        if self.null_textures.contains(name) {
            return None;
        }
        if let Some(t) = self.shared.get(name) {
            return Some(t.clone());
        }
        let mut subname = name.replace('\\', "/");
        if !subname.ends_with(".txt") && !subname.contains("/mods/") {
            if subname.ends_with(".png") {
                subname.truncate(subname.rfind('.').unwrap_or(subname.len()));
            }
            let subname = subname[subname.rfind('/').map_or(0, |i| i + 1)..].to_string();
            // TexturePackPage.getTexture(subname): the old pack pages' subTextureMap, empty (every pack goes through
            // FileSystem.mountTexturePack)
            if let Some(sub) = self.texture_pack_textures.get(&subname).cloned() {
                let t = self.load_sub_texture(&sub);
                self.shared.insert(name.to_string(), t.clone());
                return Some(t);
            }
        }
        if let Some(sub) = self.texture_pack_textures.get(name).cloned() {
            let t = self.load_sub_texture(&sub);
            self.shared.insert(name.to_string(), t.clone());
            return Some(t);
        }
        if let Some(t) = self.bucket.get(name).cloned() {
            self.shared.insert(name.to_string(), t.clone());
            return Some(t);
        }
        if !name.contains('.') {
            self.null_textures.insert(name.to_string());
            return None;
        }
        let path = match fs.get_string(name) {
            Some(p) => p.to_string(),
            None => {
                if !self.root.join(name).exists() {
                    self.null_textures.insert(name.to_string());
                    return None;
                }
                name.to_string()
            }
        };
        let t = self
            .assets
            .entry(path.clone())
            .or_insert_with(|| Rc::new(RefCell::new(Texture::from_file(&path))))
            .clone();
        self.bucket.insert(name.to_string(), t.clone());
        self.shared.insert(name.to_string(), t.clone());
        Some(t)
    }
}
