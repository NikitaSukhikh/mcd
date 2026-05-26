# Orion Apex Motors Technical Specification Dossier

This technical specification dossier covers the Orion Apex Motors (OAM) vehicle program across six manufacturing plants: Detroit (DET1), Ontario (ONT1), Saxony (SAX4), Brunswick (BRN3), Kobe (KOB1), and Mexico (MEX2). The data is synthetic, but the structure follows common OEM practice: platform and variant records define vehicle attributes, certification is market-specific, powertrain calibrations are released by engineering part number, battery packs carry both gross and usable energy, chassis tests record engineering validation outcomes, and production quality lots track manufacturing release gates.

The dossier intentionally distinguishes regulatory evidence from internal engineering targets. Regulatory records depend on market, test cycle, procedure, and approval authority. Internal targets cover towing, braking, thermal performance, durability, manufacturability, and quality maturity. A released production lot requires applicable engineering release, validation completion, and quality-gate evidence; no single field is treated as a universal global standard.

## Engineering Calculation Basis

Aerodynamic road load is estimated as `Fd = 0.5 * rho * Cd * A * v^2`, using `drag_coefficient` and `frontal_area_m2` from the vehicle variant table. This supports range and performance estimates, but it is not sufficient by itself for certification range or fuel-consumption reporting. Cycle results also depend on rolling resistance, vehicle mass, tyres, drivetrain efficiency, auxiliary load, thermal conditions, and drive-cycle speed trace.

Powertrain output is sanity-checked with `P_kW = T_Nm * rpm / 9549` where a speed point is available. The table records calibration-level `peak_power_kw`, `peak_torque_nm`, `final_drive_ratio`, software revision, emissions output, and production release date. `wltp_co2_g_per_km` is retained as a cycle-specific CO2 result for WLTP-like records; market-specific interpretation is controlled by the variant-level `certification_market`, `test_cycle`, and `procedure_standard` fields.

Battery gross energy is calculated from cell topology as `E_gross_kWh = series_cells * parallel_cells * cell_nominal_voltage_v * cell_capacity_ah / 1000`. The table also records `usable_capacity_kwh`, because range, towing duty cycle, and warranty calculations should use usable energy rather than gross nameplate capacity. `nominal_voltage_v` is the pack nominal voltage, equal to `series_cells * cell_nominal_voltage_v`.

Thermal checks use coolant heat transport as one input: `Q = mass_flow * cp * deltaT`. Production sizing also requires heat generation from cell internal resistance, motor and inverter losses, chiller or radiator capacity, coolant inlet temperature, ambient test case, allowed cell temperature gradient, and derating strategy. The fields `battery_heat_rejection_kw` and `thermal_derate_start_c` provide practical sidecar values for this synthetic package.

Brake and chassis validation uses vehicle test mass, GVWR, and where applicable GCWR. A 100-0 km/h stop distance is a useful engineering metric, but it is not a universal regulatory threshold. The chassis table therefore separates stopping-distance measurements from `regulatory_brake_pass`, `fade_test_pass`, `gcwr_stop_distance_m`, `trailer_stability_pass`, and `grade_launch_pass`.

Production quality metrics are interpreted as OAM internal controls, not universal industry limits. Capability is represented with `cpk_min` and `ppk_min`; measurement-system quality is represented with `msa_grr_pct`; production release depends on PPAP status, containment status, traceability, end-of-line pass rate, and defect metrics.

## Specification Notes

Each `platform_code` (PX29 through PX95) can support multiple body styles and drivetrains, subject to packaging, mass, cooling, electrical architecture, and plant tooling. The `homologation_code` prefix matches the `region` field and identifies the local approval record. The `certification_market`, `test_cycle`, and `procedure_standard` fields clarify whether the row is interpreted against WLTP, EPA, WLTC-Japan/Korea, ADR, GSO/SASO, or another local procedure.

Towing capability is validated at vehicle-combination level. The dataset uses `gcwr_kg`, `braked_trailer_rating_kg`, `unbraked_trailer_rating_kg`, chassis stability checks, grade-launch checks, brake validation, hitch structure assumptions, cooling capacity, and payload margin. Final-drive ratio and battery capacity can influence the result, but neither is used as a standalone pass/fail rule.

Curb mass represents a complete vehicle with standard equipment and the installed traction battery where applicable. Battery `mass_kg` is not added again to `curb_mass_kg`. Brake and dynamics calculations should use curb mass, payload, GVWR, or GCWR depending on the test condition.

Battery chemistry is selected by duty cycle rather than trim label alone. LFP and LMFP packs are common for cost-sensitive and cycle-life-focused applications; NMC622 and NMC811 packs are used where energy density, peak power, or packaging constraints are dominant. Performance validation is driven by discharge capability, thermal derating, inverter and motor limits, tyre load capacity, chassis controls, and repeated-use durability.

## Vehicle variant configuration specifications

Master configuration table defining each released vehicle variant. The `variant_id` (OAM-V prefix) is the primary key referenced by chassis validation tests. The table includes physical attributes (`body_style`, `drivetrain`, `wheelbase_mm`, `curb_mass_kg`, `drag_coefficient`, `frontal_area_m2`), payload and towing attributes, and market-certification metadata.

`tow_rating_kg` is the published braked rating for the variant where applicable and is mirrored by `braked_trailer_rating_kg`. `gcwr_kg` represents the maximum validated vehicle-plus-trailer combination mass for the synthetic data. `unbraked_trailer_rating_kg` is capped at local-market limits in the data generation assumptions. These fields provide a more realistic basis for towing validation than a fixed battery-capacity or final-drive threshold.

:::table
ref: vehicle_variant_configuration_specs-table
table: vehicle_variant_configuration_specs
view: default
display: table
caption: Vehicle variant configuration specifications
numbering: auto
:::

## Powertrain calibration specifications

Engine and hybrid-control calibrations are indexed by `calibration_id` (CAL- prefix). Each calibration is bound to a `platform_code` present in the vehicle variant table. The table records engine family, displacement, boost pressure, output, final drive, software revision, CO2 result, ECU checksum, release date, propulsion type, emissions-certification basis, and OBD standard.

The CO2 column remains named `wltp_co2_g_per_km` for compatibility with the original example, but it should be interpreted only with the linked market and procedure context. EU and UK releases use WLTP-type evidence; North American releases require EPA/CFR-style evidence; Japan, Korea, Australia, GCC, and LATAM records use their applicable local procedures.

:::table
ref: powertrain_calibration_specs-table
table: powertrain_calibration_specs
view: default
display: table
caption: Powertrain calibration specifications
numbering: auto
:::

## Battery pack and module specifications

High-voltage battery architecture records are indexed by `pack_id` (BAT- prefix). The table stores cell topology, pack voltage, gross capacity, usable capacity, module count, cell capacity, peak discharge, continuous charging capability, cooling flow, pack mass, estimated range, battery heat-rejection requirement, derate temperature, and BMS firmware version.

`estimated_range_km` is an engineering estimate based on usable capacity and assumed vehicle energy consumption. It is not a regulatory range label. Certification range must be reported by market and cycle. The data deliberately keeps pack capacity in a realistic passenger and light-commercial range rather than requiring a fixed 150 kWh minimum for pickups or vans.

:::table
ref: battery_pack_module_specs-table
table: battery_pack_module_specs
view: default
display: table
caption: Battery pack and module specifications
numbering: auto
:::

## Chassis and brake validation specifications

Vehicle dynamics validation records are indexed by `test_id` (CHS- prefix). Each row's `vehicle_variant` field references a valid `variant_id` from the vehicle configuration table and inherits mass, wheelbase, payload, towing, and certification context.

Axle configuration follows vehicle duty cycle and trim. Leaf-solid rear configurations are common on high-payload pickups and vans; air-multilink appears on premium variants where ride height and load leveling are expected; sport-oriented variants use independent layouts with higher tyre and damping targets. Stop-distance, fade, GCWR braking, trailer-stability, and grade-launch results are recorded separately so internal performance targets are not confused with statutory requirements.

:::table
ref: chassis_brake_validation_specs-table
table: chassis_brake_validation_specs
view: default
display: table
caption: Chassis and brake validation specifications
numbering: auto
:::

## Production quality measurements

Final assembly quality records are indexed by `lot_id` (LOT- prefix). Each lot is built at a specific plant and line, with plant assignment broadly aligned to regional demand and platform tooling: DET1/ONT1/MEX2 for North America and LATAM, SAX4/BRN3 for Europe and the UK, and KOB1 for Japan, Korea, Australia, and GCC demand.

Lots record inspection volume, rework PPM, paint defects, gap/flush mean, end-of-line pass rate, water-leak failures, battery-health score, warranty-risk index, capability indices, MSA result, PPAP status, containment status, and supplier traceability. `release_status=released` represents an internal lot-disposition decision after upstream engineering release and validation checks; `containment` and `hold` represent normal manufacturing-control states requiring investigation or rework before shipment.

:::table
ref: production_quality_measurements-table
table: production_quality_measurements
view: default
display: table
caption: Production quality measurements
numbering: auto
:::
