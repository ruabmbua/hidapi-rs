#![allow(dead_code)]

use std::fmt::Debug;

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(u16)]
pub enum ReportType {
    Input = 0x0,
    Output = 0x1,
    Feature = 0x2
}
impl ReportType {
    pub const fn values() -> impl IntoIterator<Item=Self> {
        [Self::Input, Self::Output, Self::Feature]
    }
}


#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub enum Items {
    MainInput               = 0x80, /* 1000 00 nn */
    MainOutput              = 0x90, /* 1001 00 nn */
    MainFeature             = 0xB0, /* 1011 00 nn */
    MainCollection          = 0xA0, /* 1010 00 nn */
    MainCollectionEnd      = 0xC0, /* 1100 00 nn */
    GlobalUsagePage        = 0x04, /* 0000 01 nn */
    GlobalLogicalMinimum   = 0x14, /* 0001 01 nn */
    GlobalLogicalMaximum   = 0x24, /* 0010 01 nn */
    GlobalPhysicalMinimum  = 0x34, /* 0011 01 nn */
    GlobalPhysicalMaximum  = 0x44, /* 0100 01 nn */
    GlobalUnitExponent     = 0x54, /* 0101 01 nn */
    GlobalUnit              = 0x64, /* 0110 01 nn */
    GlobalReportSize       = 0x74, /* 0111 01 nn */
    GlobalReportId         = 0x84, /* 1000 01 nn */
    GlobalReportCount      = 0x94, /* 1001 01 nn */
    GlobalPush              = 0xA4, /* 1010 01 nn */
    GlobalPop               = 0xB4, /* 1011 01 nn */
    LocalUsage              = 0x08, /* 0000 10 nn */
    LocalUsageMinimum      = 0x18, /* 0001 10 nn */
    LocalUsageMaximum      = 0x28, /* 0010 10 nn */
    LocalDesignatorIndex   = 0x38, /* 0011 10 nn */
    LocalDesignatorMinimum = 0x48, /* 0100 10 nn */
    LocalDesignatorMaximum = 0x58, /* 0101 10 nn */
    LocalString             = 0x78, /* 0111 10 nn */
    LocalStringMinimum     = 0x88, /* 1000 10 nn */
    LocalStringMaximum     = 0x98, /* 1001 10 nn */
    LocalDelimiter          = 0xA8  /* 1010 10 nn */
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[repr(u16)]
pub enum MainItems {
    Input = ReportType::Input as u16,
    Output = ReportType::Output as u16,
    Feature = ReportType::Feature as u16,
    Collection,
    CollectionEnd,
    DelimiterOpen,
    DelimiterUsage,
    DelimiterClose,
}

impl From<ReportType> for MainItems {
    fn from(value: ReportType) -> Self {
        match value {
            ReportType::Input => Self::Input,
            ReportType::Output => Self::Output,
            ReportType::Feature => Self::Feature
        }
    }
}

impl TryFrom<MainItems> for ReportType {
    type Error = ();

    fn try_from(value: MainItems) -> Result<Self, Self::Error> {
        match value {
            MainItems::Input => Ok(Self::Input),
            MainItems::Output => Ok(Self::Output),
            MainItems::Feature => Ok(Self::Feature),
            _ => Err(())
        }
    }
}

#[derive(Default, Copy, Clone, Eq, PartialEq)]
pub struct BitRange {
    pub first_bit: u16,
    pub last_bit: u16
}

impl BitRange {
    pub fn merge(self, other: BitRange) -> BitRange {
        BitRange {
            first_bit: self.first_bit.min(other.first_bit),
            last_bit: self.last_bit.max(other.last_bit),
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ItemNodeType {
    Cap,
    Padding,
    Collection,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct MainItemNode {
    pub first_bit: u16,
    pub last_bit: u16,
    pub node_type: ItemNodeType,
    pub caps_index: i32,
    pub collection_index: usize,
    pub main_item_type: MainItems,
    pub report_id: u8
}

impl MainItemNode {
    pub fn new(first_bit: u16, last_bit: u16, node_type: ItemNodeType, caps_index: i32, collection_index: usize, main_item_type: MainItems, report_id: u8) -> Self {
        Self { first_bit, last_bit, node_type, caps_index, collection_index, main_item_type, report_id }
    }

}
