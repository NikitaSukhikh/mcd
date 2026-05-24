# Cross-Table Dependencies

## New cross-table dependencies created

| From Table | References | Dependency Type |
| ---------- | ---------- | --------------- |
| chassis_brake_validation_specs | vehicle_variant_configuration_specs | `vehicle_variant` → `variant_id` for mass, wheelbase, tow rating |
| powertrain_calibration_specs | vehicle_variant_configuration_specs | `platform_code` must exist; emissions must match region |
| battery_pack_module_specs | powertrain_calibration_specs | `coolant_flow_l_min` must satisfy `thermal_limit_deg_c` |
| battery_pack_module_specs | vehicle_variant_configuration_specs | `estimated_range_km` validated against drag × frontal area |
| production_quality_measurements | powertrain_calibration_specs | `build_date` must follow `production_release_date` |
| production_quality_measurements | chassis_brake_validation_specs | `build_date` must follow `validation_date` |
| production_quality_measurements | battery_pack_module_specs | `battery_health_score_pct` correlates with `bms_firmware_version` |

## Business rules requiring multi-table validation

- **Plant assignments**: DET1/ONT1/MEX2 for NA, SAX4/BRN3 for EU/UK, KOB1 for APAC
- **Chemistry constraints**: LFP → Fleet trim, NMC811 → Performance trim
- **Axle config rules**: leaf-solid → pickup/van, air-multilink → Executive/Premium
- **Towing thresholds**: `tow_rating_kg` > 2000 requires `stop_distance` < 40m
- **Emissions gates**: `wltp_co2_g_per_km` > 150 blocked for EU/UK regions
