# Cross-Table Dependencies

## Declared relational metadata

| From Table | References | Dependency Type |
| ---------- | ---------- | --------------- |
| chassis_brake_validation_specs | vehicle_variant_configuration_specs | `vehicle_variant` -> `variant_id` for mass, wheelbase, payload, towing, and certification context |

## Business rules requiring multi-table validation

The remaining dependencies are domain validation rules rather than schema-level foreign keys because the source CSVs do not contain direct upstream ID columns for every relationship.

| From Table | Related Table | Rule |
| ---------- | ------------- | ---- |
| powertrain_calibration_specs | vehicle_variant_configuration_specs | `platform_code` compatibility, propulsion type, OBD evidence, and CO2 interpretation are validated against variant market, `test_cycle`, and `procedure_standard` |
| battery_pack_module_specs | vehicle_variant_configuration_specs | `usable_capacity_kwh` and `estimated_range_km` are evaluated against vehicle mass, CdA, rolling resistance, drivetrain efficiency, auxiliary load, and selected certification or engineering cycle |
| battery_pack_module_specs | powertrain_calibration_specs | `battery_heat_rejection_kw`, `coolant_flow_l_min`, and `thermal_derate_start_c` must support expected motor, inverter, and pack heat loads for the paired calibration |
| chassis_brake_validation_specs | vehicle_variant_configuration_specs | Brake, spring, damping, tyre, fade, trailer stability, and grade-launch results are checked against curb mass, payload, `gcwr_kg`, and `tow_rating_kg` |
| production_quality_measurements | powertrain_calibration_specs | `build_date` must follow the applicable `production_release_date`; released lots require valid calibration checksum and software revision traceability |
| production_quality_measurements | chassis_brake_validation_specs | `build_date` must follow completed validation for the referenced variant family and must not ship if required validation is on hold |
| production_quality_measurements | battery_pack_module_specs | `battery_health_score_pct` is trended by chemistry, `usable_capacity_kwh`, module count, and `bms_firmware_version` |

- **Plant assignments**: DET1/ONT1/MEX2 support NA and LATAM demand; SAX4/BRN3 support EU/UK demand; KOB1 supports JP/KR/AU/GCC demand.
- **Certification metadata**: `homologation_code` prefixes match `region`; `certification_market`, `test_cycle`, and `procedure_standard` define how emissions, fuel-consumption, and EV-range values are interpreted.
- **Battery energy**: `capacity_kwh` is gross nameplate energy; `usable_capacity_kwh` is used for range, warranty, and duty-cycle checks.
- **Towing validation**: towing is validated with `gcwr_kg`, braked and unbraked trailer ratings, cooling, grade launch, brake fade, trailer stability, hitch assumptions, tyre/axle capacity, and payload margin.
- **Brake validation**: `stop_distance_100_0_m` is an internal engineering result, while `regulatory_brake_pass`, `fade_test_pass`, and `gcwr_stop_distance_m` separate statutory and loaded-condition evidence.
- **Quality gates**: production release uses PPAP status, containment status, `cpk_min`, `ppk_min`, `msa_grr_pct`, traceability, end-of-line pass rate, defect PPM, battery-health trend, and warranty-risk trend.
