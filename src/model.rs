use crate::error::SQLiteError;

#[allow(dead_code)]
pub struct Database {
    pub header: DbHeader,
    pub pages: Vec<Page>,
}

pub struct DbHeader {
    pub page_size: PageSize,
    pub write_version: u8,
    pub read_version: u8,
    pub max_payload_fraction: u8,
    pub min_payload_fraction: u8,
    pub leaf_payload_fraction: u8,
    pub file_change_counter: u32,
    pub db_size: u32,
    pub first_freelist_page_no: u32,
    pub total_freelist_pages: u32,
    pub schema_cookie: u32,
    pub schema_format_no: u32,
    pub default_page_cache_size: u32,
    pub no_largest_root_b_tree: u32,
    pub db_text_encoding: TextEncoding,
    pub user_version: u32,
    pub incremental_vacuum_mode: u32,
    pub application_id: u32,
    pub version_valid_for_no: u32,
    pub sqlite_version_number: u32,
}

pub struct PageSize(pub u16);

impl PageSize {
    pub fn real_size(&self) -> usize {
        match self.0 {
            1 => 0x1_00_00,
            _ => self.0.into(),
        }
    }
}

#[derive(Copy, Clone)]
pub enum TextEncoding {
    Utf8,
    Utf16Le,
    Utf16Be,
}

impl TryFrom<u32> for TextEncoding {
    type Error = SQLiteError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        use TextEncoding::*;

        match value {
            1 => Ok(Utf8),
            2 => Ok(Utf16Le),
            3 => Ok(Utf16Be),
            _ => Err(SQLiteError::UnknownTextEncodingError(value)),
        }
    }
}

pub enum Page {
    InteriorIndex(InteriorIndexPage),
    LeafIndex(LeafIndexPage),
    InteriorTable(InteriorTablePage),
    LeafTable(LeafTablePage),
}

pub struct InteriorPageHeader {
    pub first_freeblock_offset: Option<u16>,
    pub no_cells: u16,
    pub cell_content_offset: CellOffset,
    pub no_fragmented_bytes: u8,
    pub rightmost_pointer: u32,
}

pub struct InteriorIndexPage {
    pub header: InteriorPageHeader,
    pub cell_pointers: Vec<u16>,
    pub cells: Vec<InteriorIndexCell>,
}

pub struct InteriorTablePage {
    pub header: InteriorPageHeader,
    pub cell_pointers: Vec<u16>,
    pub cells: Vec<InteriorTableCell>,
}

pub struct IndexCellPayload {
    pub header_size: u64,
    pub column_types: Vec<SerialType>,
    pub column_values: Vec<Option<Payload>>,
    pub rowid: u64,
}

pub struct InteriorIndexCell {
    pub left_child_page_no: u32,
    pub payload_size: u64,
    pub payload: IndexCellPayload,
    pub overflow_page_no: Option<u32>,
}

pub struct InteriorTableCell {
    pub left_child_page_no: u32,
    pub integer_key: u64,
}

pub struct CellOffset(pub u16);

impl CellOffset {
    pub fn real_offset(&self) -> u32 {
        match self.0 {
            0 => 0x1_00_00,
            _ => self.0.into(),
        }
    }
}

pub struct LeafPageHeader {
    pub first_freeblock_offset: Option<u16>,
    pub no_cells: u16,
    pub cell_content_offset: CellOffset,
    pub no_fragmented_bytes: u8,
}

pub struct LeafIndexPage {
    pub header: LeafPageHeader,
    pub cell_pointers: Vec<u16>,
    pub cells: Vec<LeafIndexCell>,
}

pub struct LeafIndexCell {
    pub payload_size: u64,
    pub payload: IndexCellPayload,
    pub overflow_page_no: Option<u32>,
}

pub struct LeafTablePage {
    pub header: LeafPageHeader,
    pub cell_pointers: Vec<u16>,
    pub cells: Vec<LeafTableCell>,
}

pub struct TableCellPayload {
    pub header_size: u64,
    pub column_types: Vec<SerialType>,
    pub column_values: Vec<Option<Payload>>,
}

pub struct LeafTableCell {
    pub payload_size: u64,
    pub rowid: u64,
    pub payload: TableCellPayload,
    pub overflow_page_no: Option<u32>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum SerialType {
    Null,
    I8,
    I16,
    I24,
    I32,
    I48,
    I64,
    F64,
    Const0,
    Const1,
    Reserved,
    Blob(u64),
    Text(u64),
}

impl From<u64> for SerialType {
    fn from(value: u64) -> Self {
        use SerialType::*;
        match value {
            0 => Null,
            1 => I8,
            2 => I16,
            3 => I24,
            4 => I32,
            5 => I48,
            6 => I64,
            7 => F64,
            8 => Const0,
            9 => Const1,
            10 | 11 => Reserved,
            n if n >= 12 && n % 2 == 0 => Blob(n),
            n if n >= 13 && n % 2 == 1 => Text(n),
            _ => unreachable!(),
        }
    }
}

impl SerialType {
    pub fn size(&self) -> usize {
        match self {
            SerialType::Null => 0,
            SerialType::I8 => 1,
            SerialType::I16 => 2,
            SerialType::I24 => 3,
            SerialType::I32 => 4,
            SerialType::I48 => 6,
            SerialType::I64 => 8,
            SerialType::F64 => 8,
            SerialType::Const0 => 0,
            SerialType::Const1 => 0,
            SerialType::Reserved => unimplemented!("reserved"),
            SerialType::Blob(n) => ((n - 12) / 2).try_into().unwrap(),
            SerialType::Text(n) => ((n - 13) / 2).try_into().unwrap(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Payload {
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    F64(f64),
    Blob(Vec<u8>),
    Text(String),
}

#[cfg(test)]
impl Eq for Payload {}
