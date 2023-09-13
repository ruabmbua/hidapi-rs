use crate::windows_native::descriptor::typedefs::{Caps, LinkCollectionNode};
use crate::windows_native::descriptor::types::{ItemNodeType, Items, MainItemNode, MainItems};
use crate::windows_native::error::{WinError, WinResult};
use crate::windows_native::utils::PeakIterExt;

pub fn encode_descriptor(main_item_list: &[MainItemNode], caps_list: &[Caps], link_collection_nodes: &[LinkCollectionNode]) -> WinResult<Vec<u8>> {
    // ***********************************
    // Encode the report descriptor output
    // ***********************************

    let mut writer = DescriptorWriter::default();

    let mut last_report_id = 0;
    let mut last_usage_page = 0;
    let mut last_physical_min = 0;// If both, Physical Minimum and Physical Maximum are 0, the logical limits should be taken as physical limits according USB HID spec 1.11 chapter 6.2.2.7
    let mut last_physical_max = 0;
    let mut last_unit_exponent = 0; // If Unit Exponent is Undefined it should be considered as 0 according USB HID spec 1.11 chapter 6.2.2.7
    let mut last_unit = 0; // If the first nibble is 7, or second nibble of Unit is 0, the unit is None according USB HID spec 1.11 chapter 6.2.2.7
    let mut inhibit_write_of_usage = false; // Needed in case of delimited usage print, before the normal collection or cap
    let mut report_count = 0;

    for (current, next) in main_item_list.iter().peaking() {
        let rt_idx = current.main_item_type;
        let caps_idx = current.caps_index;
        match current.main_item_type {
            MainItems::Collection => {
                if last_usage_page != link_collection_nodes[current.collection_index].link_usage_page {
                    // Write "Usage Page" at the begin of a collection - except it refers the same table as wrote last
                    last_usage_page = link_collection_nodes[current.collection_index].link_usage_page;
                    writer.write(Items::GlobalUsagePage, last_usage_page)?;
                }
                if inhibit_write_of_usage {
                    // Inhibit only once after DELIMITER statement
                    inhibit_write_of_usage = false;
                } else {
                    // Write "Usage" of collection
                    writer.write(Items::LocalUsage, link_collection_nodes[current.collection_index].link_usage)?;
                }
                // Write begin of "Collection"
                writer.write(Items::MainCollection, link_collection_nodes[current.collection_index].collection_type())?;
            }
            MainItems::CollectionEnd => {
                // Write "End Collection"
                writer.write(Items::MainCollectionEnd, 0)?;
            }
            MainItems::DelimiterOpen => {
                // Current.collection_index seems to always be != -1 -> removing branches compared to c
                // Write "Usage Page" inside of a collection delmiter section
                if last_usage_page != link_collection_nodes[current.collection_index].link_usage_page {
                    last_usage_page = link_collection_nodes[current.collection_index].link_usage_page;
                    writer.write(Items::GlobalUsagePage, link_collection_nodes[current.collection_index].collection_type())?;
                }
                // Write "Delimiter Open"
                writer.write(Items::LocalDelimiter, 1)?; // 1 = open set of aliased usages
            }
            MainItems::DelimiterUsage => {
                // Current.collection_index seems to always be != -1 -> removing branches compared to c
                // Write aliased collection "Usage"
                writer.write(Items::LocalUsage, link_collection_nodes[current.collection_index].link_usage)?;
            }
            MainItems::DelimiterClose => {
                // Write "Delimiter Close"
                writer.write(Items::LocalDelimiter, 0)?; // 0 = close set of aliased usages
                // Inhibit next usage write
                inhibit_write_of_usage = true;
            },
            _ if current.node_type == ItemNodeType::Padding => {
                // Padding
                // The preparsed data doesn't contain any information about padding. Therefore all undefined gaps
                // in the reports are filled with the same style of constant padding.

                // Write "Report Size" with number of padding bits
                writer.write(Items::GlobalReportSize, current.last_bit - current.first_bit + 1)?;

                // Write "Report Count" for padding always as 1
                writer.write(Items::GlobalReportCount, 1)?;

                if rt_idx == MainItems::Input {
                    // Write "Input" main item - We know it's Constant - We can only guess the other bits, but they don't matter in case of const
                    writer.write(Items::MainInput, 0x03)?; // Const / Abs
                } else if rt_idx == MainItems::Output {
                    // Write "Output" main item - We know it's Constant - We can only guess the other bits, but they don't matter in case of const
                    writer.write(Items::MainOutput, 0x03)?; // Const / Abs
                } else if rt_idx == MainItems::Feature {
                    // Write "Feature" main item - We know it's Constant - We can only guess the other bits, but they don't matter in case of const
                    writer.write(Items::MainFeature, 0x03)?; // Const / Abs
                }
                report_count = 0;
            },
            _ if caps_list[caps_idx as usize].is_button_cap() => {
                let caps = caps_list[caps_idx as usize];
                // Button
                // (The preparsed data contain different data for 1 bit Button caps, than for parametric Value caps)

                if last_report_id != caps.report_id {
                    // Write "Report ID" if changed
                    last_report_id = caps.report_id;
                    writer.write(Items::GlobalReportId, last_report_id)?;
                }

                // Write "Usage Page" when changed
                if caps.usage_page != last_usage_page {
                    last_usage_page = caps.usage_page;
                    writer.write(Items::GlobalUsagePage, last_usage_page)?;
                }

                // Write only local report items for each cap, if ReportCount > 1
                if caps.is_range() {
                    report_count += caps.range().data_index_max - caps.range().data_index_min;
                }

                if inhibit_write_of_usage {
                    // Inhibit only once after Delimiter - Reset flag
                    inhibit_write_of_usage = false;
                } else if caps.is_range() {
                    // Write range from "Usage Minimum" to "Usage Maximum"
                    writer.write(Items::LocalUsageMinimum, caps.range().usage_min)?;
                    writer.write(Items::LocalUsageMaximum, caps.range().usage_max)?;
                } else {
                    // Write single "Usage"
                    writer.write(Items::LocalUsage, caps.not_range().usage)?;
                }


                if caps.is_designator_range() {
                    // Write physical descriptor indices range from "Designator Minimum" to "Designator Maximum"
                    writer.write(Items::LocalDesignatorMinimum, caps.range().designator_min)?;
                    writer.write(Items::LocalDesignatorMaximum, caps.range().designator_max)?;
                } else if caps.not_range().designator_index != 0 {
                    // Designator set 0 is a special descriptor set (of the HID Physical Descriptor),
                    // that specifies the number of additional descriptor sets.
                    // Therefore Designator Index 0 can never be a useful reference for a control and we can inhibit it.
                    // Write single "Designator Index"
                    writer.write(Items::LocalDesignatorIndex, caps.not_range().designator_index)?;
                }

                if caps.is_string_range() {
                    // Write range of indices of the USB string descriptor, from "String Minimum" to "String Maximum"
                    writer.write(Items::LocalStringMinimum, caps.range().string_min)?;
                    writer.write(Items::LocalStringMaximum, caps.range().string_max)?;
                } else if caps.not_range().string_index != 0 {
                    // String Index 0 is a special entry of the USB string descriptor, that contains a list of supported languages,
                    // therefore Designator Index 0 can never be a useful reference for a control and we can inhibit it.
                    // Write single "String Index"
                    writer.write(Items::LocalString, caps.not_range().string_index)?;
                }

                if next.is_some_and(|next|
                    next.main_item_type == rt_idx &&
                        next.node_type == ItemNodeType::Cap &&
                        !caps.is_range() && // This node in list is no array
                        !caps_list[next.caps_index as usize].is_range() && // Next node in list is no array
                        caps_list[next.caps_index as usize].is_button_cap() &&
                        caps_list[next.caps_index as usize].usage_page == caps.usage_page &&
                        caps_list[next.caps_index as usize].report_id == caps.report_id &&
                        caps_list[next.caps_index as usize].bit_field == caps.bit_field ){

                    if next.unwrap().first_bit != current.first_bit {
                        // In case of IsMultipleItemsForArray for multiple dedicated usages for a multi-button array, the report count should be incremented

                        // Skip global items until any of them changes, than use ReportCount item to write the count of identical report fields
                        report_count += 1;
                    }
                } else {
                    if caps.button().logical_min == 0 &&
                        caps.button().logical_max == 0 {
                        // While a HID report descriptor must always contain LogicalMinimum and LogicalMaximum,
                        // the preparsed data contain both fields set to zero, for the case of simple buttons
                        // Write "Logical Minimum" set to 0 and "Logical Maximum" set to 1
                        writer.write(Items::GlobalLogicalMinimum, 0)?;
                        writer.write(Items::GlobalLogicalMaximum, 1)?;
                    } else {
                        // Write logical range from "Logical Minimum" to "Logical Maximum"
                        writer.write(Items::GlobalLogicalMinimum, caps.button().logical_min)?;
                        writer.write(Items::GlobalLogicalMaximum, caps.button().logical_max)?;
                    }

                    // Write "Report Size"
                    writer.write(Items::GlobalReportSize, caps.report_size)?;

                    // Write "Report Count"
                    if !caps.is_range() {
                        // Variable bit field with one bit per button
                        // In case of multiple usages with the same items, only "Usage" is written per cap, and "Report Count" is incremented
                        writer.write(Items::GlobalReportCount, caps.report_count + report_count)?;
                    } else {
                        // Button array of "Report Size" x "Report Count
                        writer.write(Items::GlobalReportCount, caps.report_count)?;
                    }

                    // Buttons have only 1 bit and therefore no physical limits/units -> Set to undefined state
                    if last_physical_min != 0 {
                        // Write "Physical Minimum", but only if changed
                        last_physical_min = 0;
                        writer.write(Items::GlobalPhysicalMinimum, last_physical_min)?;
                    }
                    if last_physical_max != 0 {
                        // Write "Physical Maximum", but only if changed
                        last_physical_max = 0;
                        writer.write(Items::GlobalPhysicalMaximum, last_physical_max)?;
                    }
                    if last_unit_exponent != 0 {
                        // Write "Unit Exponent", but only if changed
                        last_unit_exponent = 0;
                        writer.write(Items::GlobalUnitExponent, last_unit_exponent)?;
                    }
                    if last_unit != 0 {
                        // Write "Unit",but only if changed
                        last_unit = 0;
                        writer.write(Items::GlobalUnit, last_unit)?;
                    }

                    // Write "Input" main item
                    if rt_idx == MainItems::Input {
                        writer.write(Items::MainInput, caps.bit_field)?;
                    }
                    // Write "Output" main item
                    else if rt_idx == MainItems::Output {
                        writer.write(Items::MainOutput, caps.bit_field)?;
                    }
                    // Write "Feature" main item
                    else if rt_idx == MainItems::Feature {
                        writer.write(Items::MainFeature, caps.bit_field)?;
                    }
                    report_count = 0;
                }

            }
            _ => {
                let mut caps = caps_list[caps_idx as usize];

                if last_report_id != caps.report_id {
                    // Write "Report ID" if changed
                    last_report_id = caps.report_id;
                    writer.write(Items::GlobalReportId, last_report_id)?;
                }

                // Write "Usage Page" if changed
                if caps.usage_page != last_usage_page {
                    last_usage_page = caps.usage_page;
                    writer.write(Items::GlobalUsagePage, last_usage_page)?;
                }

                if inhibit_write_of_usage {
                    // Inhibit only once after Delimiter - Reset flag
                    inhibit_write_of_usage = false;
                } else if caps.is_range() {
                    // Write usage range from "Usage Minimum" to "Usage Maximum"
                    writer.write(Items::LocalUsageMinimum, caps.range().usage_min)?;
                    writer.write(Items::LocalUsageMaximum, caps.range().usage_max)?;
                } else {
                    // Write single "Usage"
                    writer.write(Items::LocalUsage, caps.not_range().usage)?;
                }


                if caps.is_designator_range() {
                    // Write physical descriptor indices range from "Designator Minimum" to "Designator Maximum"
                    writer.write(Items::LocalDesignatorMinimum, caps.range().designator_min)?;
                    writer.write(Items::LocalDesignatorMaximum, caps.range().designator_max)?;
                } else if caps.not_range().designator_index != 0 {
                    // Designator set 0 is a special descriptor set (of the HID Physical Descriptor),
                    // that specifies the number of additional descriptor sets.
                    // Therefore Designator Index 0 can never be a useful reference for a control and we can inhibit it.
                    // Write single "Designator Index"
                    writer.write(Items::LocalDesignatorIndex, caps.not_range().designator_index)?;
                }

                if caps.is_string_range() {
                    // Write range of indices of the USB string descriptor, from "String Minimum" to "String Maximum"
                    writer.write(Items::LocalStringMinimum, caps.range().string_min)?;
                    writer.write(Items::LocalStringMaximum, caps.range().string_max)?;
                } else if caps.not_range().string_index != 0 {
                    // String Index 0 is a special entry of the USB string descriptor, that contains a list of supported languages,
                    // therefore Designator Index 0 can never be a useful reference for a control and we can inhibit it.
                    // Write single "String Index"
                    writer.write(Items::LocalString, caps.not_range().string_index)?;
                }

                if (caps.bit_field & 0x02) != 0x02 {
                    // In case of an value array overwrite "Report Count"
                    caps.report_count = caps.range().data_index_max - caps.range().data_index_min + 1;
                }

                #[allow(clippy::blocks_in_if_conditions)]
                if next.is_some_and(|next| {
                    let next_caps = caps_list
                        .get(next.caps_index as usize)
                        .copied()
                        .unwrap_or_else(|| unsafe {std::mem::zeroed()});
                    next.main_item_type == rt_idx &&
                    next.node_type == ItemNodeType::Cap &&
                    !next_caps.is_button_cap() &&
                    !caps.is_range() &&
                    !next_caps.is_range() &&
                    next_caps.usage_page == caps.usage_page &&
                    next_caps.not_button().logical_min == caps.not_button().logical_min &&
                    next_caps.not_button().logical_max == caps.not_button().logical_max &&
                    next_caps.not_button().physical_min == caps.not_button().physical_min &&
                    next_caps.not_button().physical_max == caps.not_button().physical_max &&
                    next_caps.units_exp == caps.units_exp &&
                    next_caps.units == caps.units &&
                    next_caps.report_size == caps.report_size &&
                    next_caps.report_id == caps.report_id &&
                    next_caps.bit_field == caps.bit_field &&
                    next_caps.report_count == 1 &&
                    caps.report_count == 1
                }) {
                    // Skip global items until any of them changes, than use ReportCount item to write the count of identical report fields
                    report_count += 1;
                } else {
                    // Value

                    // Write logical range from "Logical Minimum" to "Logical Maximum"
                    writer.write(Items::GlobalLogicalMinimum, caps.not_button().logical_min)?;
                    writer.write(Items::GlobalLogicalMaximum, caps.not_button().logical_max)?;

                    if (last_physical_min != caps.not_button().physical_min) || (last_physical_max != caps.not_button().physical_max) {
                        // Write range from "Physical Minimum" to " Physical Maximum", but only if one of them changed
                        last_physical_min = caps.not_button().physical_min;
                        last_physical_max = caps.not_button().physical_max;
                        writer.write(Items::GlobalPhysicalMinimum, last_physical_min)?;
                        writer.write(Items::GlobalPhysicalMaximum, last_physical_max)?;
                    }

                    if last_unit_exponent != caps.units_exp {
                        // Write "Unit Exponent", but only if changed
                        last_unit_exponent = caps.units_exp;
                        writer.write(Items::GlobalUnitExponent, last_unit_exponent)?;
                    }

                    if last_unit != caps.units {
                        // Write physical "Unit", but only if changed
                        last_unit = caps.units;
                        writer.write(Items::GlobalUnit, last_unit)?;
                    }

                    // Write "Report Size"
                    writer.write(Items::GlobalReportSize, caps.report_size)?;

                    // Write "Report Count"
                    writer.write(Items::GlobalReportCount, caps.report_count + report_count)?;

                    if rt_idx == MainItems::Input {
                        // Write "Input" main item
                        writer.write(Items::MainInput, caps.bit_field)?;
                    } else if rt_idx == MainItems::Output {
                        // Write "Output" main item
                        writer.write(Items::MainOutput, caps.bit_field)?;
                    } else if rt_idx == MainItems::Feature {
                        // Write "Feature" main item
                        writer.write(Items::MainFeature, caps.bit_field)?;
                    }
                    report_count = 0;
                }

            }
        }
    }

    Ok(writer.finish())

}

#[derive(Default)]
struct DescriptorWriter(Vec<u8>);

impl DescriptorWriter {

    // Writes a short report descriptor item according USB HID spec 1.11 chapter 6.2.2.2
    fn write(&mut self, item: Items, data: impl Into<i64>) -> WinResult<()> {
        let data = data.into();
        match item {
            Items::MainCollectionEnd => {
                self.0.push(item as u8);
            },
            Items::GlobalLogicalMinimum | Items::GlobalLogicalMaximum | Items::GlobalPhysicalMinimum | Items::GlobalPhysicalMaximum => {
                if let Ok(data) = i8::try_from(data) {
                    self.0.push((item as u8) + 0x01);
                    self.0.extend(data.to_le_bytes())
                } else if let Ok(data) = i16::try_from(data) {
                    self.0.push((item as u8) + 0x02);
                    self.0.extend(data.to_le_bytes())
                } else if let Ok(data) = i32::try_from(data) {
                    self.0.push((item as u8) + 0x03);
                    self.0.extend(data.to_le_bytes())
                } else {
                    return Err(WinError::InvalidPreparsedData)
                }
            },
            _=> {
                if let Ok(data) = u8::try_from(data) {
                    self.0.push((item as u8) + 0x01);
                    self.0.extend(data.to_le_bytes())
                } else if let Ok(data) = u16::try_from(data) {
                    self.0.push((item as u8) + 0x02);
                    self.0.extend(data.to_le_bytes())
                } else if let Ok(data) = u32::try_from(data) {
                    self.0.push((item as u8) + 0x03);
                    self.0.extend(data.to_le_bytes())
                } else {
                    return Err(WinError::InvalidPreparsedData)
                }
            }
        }
        Ok(())
    }

    fn finish(self) -> Vec<u8> {
        self.0
    }

}

