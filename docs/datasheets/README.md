# Hardware references

These PDFs are local copies of the component references used by the firmware.
The URLs record the original publisher or distributor source. Check the source
for revisions before designing new hardware around a component.

| File | Component | Source |
| --- | --- | --- |
| `ESP32-S3.pdf` | Espressif ESP32-S3 Series Datasheet v2.2 | [Espressif](https://www.espressif.com/sites/default/files/documentation/esp32-s3_datasheet_en.pdf) |
| `CO5300_Datasheet_V0.00.pdf` | Chipone CO5300 AMOLED controller | [Espressif mirror](https://dl.espressif.com/AE/esp-iot-solution/CO5300_Datasheet_V0.00.pdf) |
| `QMI8658C.pdf` | QST QMI8658C IMU | [Waveshare mirror](https://files.waveshare.com/wiki/common/QMI8658C.pdf) |
| `PCF85063A.pdf` | NXP PCF85063A RTC | [Waveshare mirror](https://files.waveshare.com/wiki/common/PCF85063A.pdf) |
| `LC76G_Series_GNSS_Specification_V1.1.pdf` | Quectel LC76G GNSS module | [Quectel](https://quectel.com/content/uploads/2024/03/Quectel_LC76G_Series_GNSS_Specification_V1.1-1-1.pdf) |
| `LC76G_I2C_Application_Note_V1.0.pdf` | Quectel LC76G I²C protocol | [Quectel](https://www.quectel.com/content/uploads/2024/02/Quectel_LC26GABLC76G_Series_I2C_Application_Note_V1.0.pdf) |
| `LC76G_Low_Power_Mode_Application_Note_V1.0.pdf` | Quectel LC76G low-power modes | [Quectel](https://quectel.com/content/uploads/2024/02/Quectel_LC26GLC76GLC86G_Series_Low_Power_Mode_Application_Note_V1.0.pdf) |
| `LC76G_AGNSS_Application_Note_V1.1.pdf` | Quectel LC76G assistance data: EPO, EASY, EPOC, reference time and position | [DigiKey mirror](https://mm.digikey.com/Volume0/opasdata/d220001/medias/docus/6504/LC26G-LC26G-T-LC76G-LC86G_Series_AGNSS_Application_Note_V1.1.pdf) |
| `LC76G_GNSS_Protocol_Specification_V1.1.pdf` | Quectel LC76G command protocol | [Quectel](https://quectel.com/content/uploads/2024/02/Quectel_LC26GABLC76GLC86G_Series_GNSS_Protocol_Specification_V1.1.pdf) |
| `BMM350_DS001.pdf` | Bosch BMM350 magnetometer | [Bosch Sensortec](https://www.bosch-sensortec.com/media/boschsensortec/downloads/datasheets/bst-bmm350-ds001.pdf) |
| `RF-LORA-868-SO.pdf` | RF Solutions RF-LORA-868-SO, SX1272-based module | [DigiKey datasheet mirror](https://mm.digikey.com/Volume0/opasdata/d220001/medias/docus/1028/RF-LORA.pdf) |
| `SX1272_Datasheet.pdf` | Semtech SX1272/73 LoRa transceiver | [Semtech product page](https://www.semtech.com/products/wireless-rf/lora-connect/sx1272) |
| `SX1272_73_Errata.pdf` | Semtech SX1272/73 V2b errata note | [Semiconductor mirror](https://www.micro-semiconductor.se/datasheet/b2-SX1272DVK1BAS.pdf) |
| `ENG_DS_2195835_A1.pdf` | TE Connectivity 2195835 ISM 868/915 MHz flexible PCB antenna; the board uses 2195835-3 | [TE product page](https://www.te.com/en/product-2195835-3.html) |
| `AXP2101_SWcharge_V1.0.pdf` | X-Powers AXP2101 power-management IC | [Waveshare mirror](https://files.waveshare.com/wiki/common/X-power-AXP2101_SWcharge_V1.0.pdf) |

The board schematic and board-level pin map remain in the Waveshare
documentation rather than this directory.

There is no CST9217 reference here. The only copy found online repeats its first page, a feature
list, and no register map has been published. Hynitron's own driver, `hyn_cst92xx.c`, is the
reference for its commands; `docs/hardware-notes.md` lists them.
