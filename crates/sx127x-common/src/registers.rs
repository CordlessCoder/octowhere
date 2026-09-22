pub const FIFO: u8 = 0x00;

// RegOpMode ---------------------------------------------------------------------------------------
pub const OP_MODE: u8 = 0x01;
pub const OP_MODE_LONG_RANGE_MODE_MASK: u8 = 0x80;
pub const OP_MODE_LONG_RANGE_MODE_OFFSET: u8 = 0x7;
pub const OP_MODE_MODULATION_TYPE_MASK: u8 = 0x60;
pub const OP_MODE_LOW_FREQUENCY_MODE_ON_MASK: u8 = 0x08;
pub const OP_MODE_LOW_FREQUENCY_MODE_ON_OFFSET: u8 = 0x03;
pub const OP_MODE_MODE_MASK: u8 = 0x07;
pub const OP_MODE_MODE_OFFSET: u8 = 0x0;

// -------------------------------------------------------------------------------------------------
pub const FRF_MSB: u8 = 0x06;
pub const FRF_MID: u8 = 0x07;
pub const FRF_LSB: u8 = 0x08;

// RegPaConfig -------------------------------------------------------------------------------------
pub const PA_CONFIG: u8 = 0x09;
pub const PA_CONFIG_PA_SELECT_MASK: u8 = 0x80;
pub const PA_CONFIG_PA_SELECT_OFFSET: u8 = 0x7;
pub const PA_CONFIG_MAX_POWER_MASK: u8 = 0x70;
pub const PA_CONFIG_MAX_POWER_OFFSET: u8 = 0x4;
pub const PA_CONFIG_OUTPUT_POWER_MASK: u8 = 0xf;
pub const PA_CONFIG_OUTPUT_POWER_OFFSET: u8 = 0x0;

// RegPaRamp ---------------------------------------------------------------------------------------
pub const PA_RAMP: u8 = 0x0a;
pub const PA_RAMP_MASK: u8 = 0xf;
pub const PA_RAMP_OFFSET: u8 = 0x0;

// RegOcp ------------------------------------------------------------------------------------------
pub const OCP: u8 = 0x0b;
pub const OCP_ON_MASK: u8 = 0x20;
pub const OCP_ON_OFFSET: u8 = 0x5;
pub const OCP_TRIM_MASK: u8 = 0x1f;
pub const OCP_TRIM_OFFSET: u8 = 0x0;

// RegLna ------------------------------------------------------------------------------------------
pub const LNA: u8 = 0x0c;
pub const LNA_GAIN_MASK: u8 = 0xe0;
pub const LNA_GAIN_OFFSET: u8 = 0x5;
pub const LNA_BOOST_LF_MASK: u8 = 0x18;
pub const LNA_BOOST_LF_OFFSET: u8 = 0x3;
pub const LNA_BOOST_HF_MASK: u8 = 0x3;
pub const LNA_BOOST_HF_OFFSET: u8 = 0x0;

// RegDioMapping1 ----------------------------------------------------------------------------------
pub const DIO_MAPPING_1: u8 = 0x40;
pub const DIO_MAPPING_1_DIO0_MASK: u8 = 0xc0;
pub const DIO_MAPPING_1_DIO0_OFFSET: u8 = 0x6;
pub const DIO_MAPPING_1_DIO1_MASK: u8 = 0x30;
pub const DIO_MAPPING_1_DIO1_OFFSET: u8 = 0x4;
pub const DIO_MAPPING_1_DIO2_MASK: u8 = 0x0c;
pub const DIO_MAPPING_1_DIO2_OFFSET: u8 = 0x2;
pub const DIO_MAPPING_1_DIO3_MASK: u8 = 0x03;
pub const DIO_MAPPING_1_DIO3_OFFSET: u8 = 0x0;

// RegDioMapping2 ----------------------------------------------------------------------------------
pub const DIO_MAPPING_2: u8 = 0x41;
pub const DIO_MAPPING_2_DIO4_MASK: u8 = 0xc0;
pub const DIO_MAPPING_2_DIO4_OFFSET: u8 = 0x6;
pub const DIO_MAPPING_2_DIO5_MASK: u8 = 0x30;
pub const DIO_MAPPING_2_DIO5_OFFSET: u8 = 0x4;

// -------------------------------------------------------------------------------------------------
pub const VERSION: u8 = 0x42;
pub const TCXO: u8 = 0x4b;

// RegPaDac ----------------------------------------------------------------------------------------
pub const PA_DAC: u8 = 0x4d;
pub const PA_DAC_MASK: u8 = 0x07;
pub const PA_DAC_OFFSET: u8 = 0x0;

// -------------------------------------------------------------------------------------------------
pub const FORMER_TEMP: u8 = 0x5b;

// RegAgcRef ---------------------------------------------------------------------------------------
pub const AGC_REF: u8 = 0x61;
pub const AGC_REF_AGC_REFERENCE_LEVEL_MASK: u8 = 0x3f;
pub const AGC_REF_AGC_REFERENCE_LEVEL_OFFSET: u8 = 0x0;

// RegAgcThresh1 -----------------------------------------------------------------------------------
pub const AGC_THRESH_1: u8 = 0x62;
pub const AGC_THRESH_1_AGC_STEP_1_MASK: u8 = 0x14;
pub const AGC_THRESH_1_AGC_STEP_1_OFFSET: u8 = 0x0;

// RegAgcThresh2 -----------------------------------------------------------------------------------
pub const AGC_THRESH_2: u8 = 0x63;
pub const AGC_THRESH_2_AGC_STEP_2_MASK: u8 = 0xf0;
pub const AGC_THRESH_2_AGC_STEP_2_OFFSET: u8 = 0x4;
pub const AGC_THRESH_2_AGC_STEP_3_MASK: u8 = 0xf;
pub const AGC_THRESH_2_AGC_STEP_3_OFFSET: u8 = 0x0;

// RegAgcThresh3 -----------------------------------------------------------------------------------
pub const AGC_THRESH_3: u8 = 0x64;
pub const AGC_THRESH_3_AGC_STEP_4_MASK: u8 = 0xf0;
pub const AGC_THRESH_3_AGC_STEP_4_OFFSET: u8 = 0x4;
pub const AGC_THRESH_3_AGC_STEP_5_MASK: u8 = 0xf;
pub const AGC_THRESH_3_AGC_STEP_5_OFFSET: u8 = 0x0;

// RegPll ------------------------------------------------------------------------------------------
pub const PLL: u8 = 0x70;
pub const PLL_PLL_BANDWIDTH_MASK: u8 = 0xc0;
pub const PLL_PLL_BANDWIDTH_OFFSET: u8 = 0x6;
