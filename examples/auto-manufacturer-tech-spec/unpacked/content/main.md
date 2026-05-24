# Orion Apex Motors Technical Specification Dossier

This technical specification dossier covers the complete Orion Apex Motors (OAM) product line across six global manufacturing plants: Detroit (DET1), Ontario (ONT1), Saxony (SAX4), Brunswick (BRN3), Kobe (KOB1), and Mexico (MEX2). Each plant specializes in specific platform codes from the PX-series architecture, with powertrain calibrations matched to regional homologation requirements.

The dossier integrates five interdependent specification domains: vehicle variant configurations define the physical envelope that constrains powertrain selection; powertrain calibrations must match platform codes declared in vehicle variants; battery pack specifications determine range figures that feed back into variant payload calculations; chassis validation tests reference specific variant IDs to ensure suspension tuning matches declared curb mass and tow ratings; production quality measurements track lot-level builds against the calibration and battery combinations released for each plant.

## Engineering Calculation Basis

Aerodynamic drag is estimated as $F_d = 0.5 \rho C_d A v^2$, using `drag_coefficient` and `frontal_area_m2` from the vehicle variant table. Variants with `active_aero=true` achieve the lower $C_d$ values required for Performance and Sport trim levels.

Powertrain power is verified with $P_{kW} = T_{Nm} \omega_{rpm} / 9549$, cross-checked against `peak_power_kw` and `peak_torque_nm` in calibration records. The V50D engine family delivers the highest output for pickup and SUV variants, while B18T and B20T four-cylinders serve sedan and wagon body styles. Tractive effort at the axle uses $F_t = T_e G_f G_g \eta / r_t$, where $G_f$ corresponds to `final_drive_ratio` in the calibration table.

Battery nominal energy follows $E_{kWh} = V_{nom} C_{Ah} N_p / 1000$, computed from `nominal_voltage_v`, `cell_capacity_ah`, and `parallel_cells`. NMC811 chemistry packs provide the highest energy density for long-range variants, while LFP packs are specified for Fleet trim vehicles prioritizing cycle life. Pack thermal sizing uses $Q_{coolant} = \dot{m} c_p \Delta T$, where $\dot{m}$ derives from `coolant_flow_l_min` to meet `thermal_limit_deg_c` constraints from the matched powertrain calibration.

Brake energy for a single 100-0 km/h stop is $E_b = 0.5 m v^2$, using `curb_mass_kg` from the variant table. Required average deceleration $a = v^2 / (2s)$ is validated against `stop_distance_100_0_m` in chassis tests, which must reference a valid `vehicle_variant` ID to inherit mass properties.

Production capability tracking uses $C_p = (USL - LSL) / (6\sigma)$, with `torque_rework_ppm` and `paint_defect_ppm` feeding the defects-per-million calculation $DPMO = defects / opportunities \times 1,000,000$. Lots achieve `release_status=released` only when `battery_health_score_pct` exceeds 96% and `warranty_risk_index` falls below 2.0.

## Specification Notes

The specification assumes a modular platform strategy where each `platform_code` (PX29 through PX95) supports multiple body styles and drivetrains. Regional homologation codes in the variant table (prefixed AU-, EU-, JP-, KR-, LATAM-, NA-, UK-) must align with the `region` field and `wltp_co2_g_per_km` emissions certification in the corresponding powertrain calibration.

Engine family assignments follow platform constraints: V30T and V35H six-cylinders pair exclusively with AWD and 4WD drivetrains; B18T and B20T four-cylinders support all drivetrain types. The `displacement_l` and `cylinders` fields in calibration records must match the engine family prefix (B=inline, V=vee configuration).

Battery pack selection depends on variant attributes: `estimated_range_km` must meet or exceed the range implied by `curb_mass_kg` and `drag_coefficient` at highway speeds. High-performance variants (lateral_grip_g > 1.0 in chassis validation) require NMC811 or NMC622 chemistry for discharge rates above 1500 kW.

Each chassis validation test (`test_id` prefix CHS-) must reference a valid `variant_id` (prefix OAM-V) to inherit wheelbase, mass, and tow rating constraints. Axle configurations correlate with body style: leaf-solid suspensions appear on pickup and van variants; air-multilink supports Executive and Premium trims with adjustable ride height.

Production lots (`lot_id` prefix LOT-) are assigned to plants based on regional demand and platform tooling. Each lot's `build_date` must fall after the `production_release_date` of its associated calibration and the `validation_date` of all referenced chassis tests.

## Vehicle variant configuration specifications

Master configuration table defining each released vehicle variant. The `variant_id` (OAM-V prefix) serves as the primary key referenced by chassis validation tests. Body style and drivetrain combinations constrain which powertrain calibrations and battery packs are compatible: pickup and van body styles require final_drive_ratio ≥ 3.5 and battery capacity_kwh ≥ 150 for adequate towing performance.

The `curb_mass_kg` field propagates to brake energy calculations in chassis validation. Variants with `tow_rating_kg` above 2000 must demonstrate stop_distance_100_0_m below 40m in their corresponding chassis tests. The `homologation_code` prefix must match the `region` field and corresponds to emissions certifications in the powertrain calibration table.

:::table
ref: vehicle_variant_configuration_specs-table
table: vehicle_variant_configuration_specs
view: default
display: table
caption: Vehicle variant configuration specifications
numbering: auto
:::

## Powertrain calibration specifications

Engine and motor control calibrations indexed by `calibration_id` (CAL- prefix). Each calibration is bound to a `platform_code` that must exist in the vehicle variant table. The `thermal_limit_deg_c` field sets the ceiling for battery pack `coolant_flow_l_min` sizing—packs paired with high-output V50D calibrations require coolant flow rates above 15 L/min.

The `final_drive_ratio` determines tractive force and must align with `tow_rating_kg` requirements: ratios below 3.0 are restricted to sedan and liftback body styles. Calibrations with `wltp_co2_g_per_km` above 150 are not released for EU or UK region variants. The `production_release_date` gates when production lots can be built; no lot's `build_date` may precede this timestamp.

:::table
ref: powertrain_calibration_specs-table
table: powertrain_calibration_specs
view: default
display: table
caption: Powertrain calibration specifications
numbering: auto
:::

## Battery pack and module specifications

High-voltage battery architecture records indexed by `pack_id` (BAT- prefix). Chemistry selection follows variant and calibration constraints: LFP packs are specified for Fleet trim variants due to superior cycle life; NMC811 packs support Performance trim vehicles requiring `peak_discharge_kw` above 1000 kW.

The `estimated_range_km` must exceed the value calculated from `capacity_kwh` and the variant's `drag_coefficient` × `frontal_area_m2` product at 100 km/h. Pack `mass_kg` adds to variant `curb_mass_kg` for total vehicle mass used in brake energy validation. The `coolant_flow_l_min` specification must satisfy thermal rejection for the paired powertrain's `thermal_limit_deg_c`. Production lots verify `battery_health_score_pct` against the pack's `bms_firmware_version` to detect anomalies.

:::table
ref: battery_pack_module_specs-table
table: battery_pack_module_specs
view: default
display: table
caption: Battery pack and module specifications
numbering: auto
:::

## Chassis and brake validation specifications

Vehicle dynamics validation records indexed by `test_id` (CHS- prefix). Each row's `vehicle_variant` field must reference a valid `variant_id` from the vehicle configuration table—this linkage inherits `curb_mass_kg`, `wheelbase_mm`, and `tow_rating_kg` for suspension and brake sizing.

The `axle_config` field correlates with body style: leaf-solid configurations are mandatory for pickup variants with `tow_rating_kg` above 2000; air-multilink suspensions pair with Executive and Premium trims. Spring rates (`front_spring_rate_n_mm`, `rear_spring_rate_n_mm`) scale with `curb_mass_kg` plus maximum `max_payload_kg`.

Brake rotor sizing (`rotor_front_mm`, `rotor_rear_mm`) must dissipate energy calculated from variant mass at 100 km/h. The `stop_distance_100_0_m` acceptance threshold tightens for Sport trim variants (must be below 35m). Validation tests with `lateral_grip_g` above 1.0 require corresponding battery packs with `peak_discharge_kw` above 1500 kW to sustain cornering loads. The `validation_date` must precede any production lot's `build_date` for that variant.

:::table
ref: chassis_brake_validation_specs-table
table: chassis_brake_validation_specs
view: default
display: table
caption: Chassis and brake validation specifications
numbering: auto
:::

## Production quality measurements

Final assembly quality records indexed by `lot_id` (LOT- prefix). Each lot is built at a specific `plant_code` (DET1, ONT1, SAX4, BRN3, KOB1, MEX2), with plant assignments determined by the variant's `region` field: NA variants build at DET1/ONT1/MEX2; EU/UK at SAX4/BRN3; JP/KR/AU/GCC at KOB1.

The `build_date` must fall after both the powertrain calibration's `production_release_date` and all chassis validation `validation_date` entries for the variant. Lots record `battery_health_score_pct` against the installed pack's `bms_firmware_version`—scores below 97% trigger correlation analysis with pack `chemistry` and `module_count`.

Quality thresholds vary by trim level: Premium and Executive variants require `gap_flush_mean_mm` below 4.0 and `paint_defect_ppm` below 1000. The `warranty_risk_index` aggregates calibration complexity (`boost_pressure_kpa`), battery stress (`peak_discharge_kw` / `capacity_kwh`), and chassis validation margin. Lots achieve `release_status=released` only when all upstream specifications pass cross-table validation checks.

:::table
ref: production_quality_measurements-table
table: production_quality_measurements
view: default
display: table
caption: Production quality measurements
numbering: auto
:::
