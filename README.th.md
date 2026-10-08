<img src="docs/images/new-conscirpt-hero-image.png" alt="ภาพหน้าปก VERA20k" width="100%">

<p align="center" dir="ltr">
  <a href="README.sv.md" lang="sv">Svenska</a> · <a href="README.zh-CN.md" lang="zh-CN">简体中文</a> · <a href="README.de.md" lang="de">Deutsch</a> · <a href="README.ar.md" lang="ar" dir="rtl">العربية</a> · <a href="README.ru.md" lang="ru">Русский</a> · <strong>ไทย</strong> · <a href="README.tr.md" lang="tr">Türkçe</a> · <a href="README.md" lang="en">English</a>
  &nbsp;&nbsp;
  <a href="https://github.com/YuriPlanet/vera20k/actions/workflows/macos.yml?query=branch%3Amain" title="การทดสอบไลบรารีบน macOS ครั้งล่าสุด (สั่งรันด้วยตนเอง)"><img src="https://github.com/YuriPlanet/vera20k/actions/workflows/macos.yml/badge.svg?branch=main" alt="การทดสอบไลบรารีบน macOS" height="20" align="middle"></a>
  <a href="https://github.com/YuriPlanet/vera20k/actions/workflows/linux.yml?query=branch%3Amain" title="การทดสอบไลบรารีบน Linux ครั้งล่าสุด (สั่งรันด้วยตนเอง)"><img src="https://github.com/YuriPlanet/vera20k/actions/workflows/linux.yml/badge.svg?branch=main" alt="การทดสอบไลบรารีบน Linux" height="20" align="middle"></a>
  <a href="https://github.com/YuriPlanet/vera20k/actions/workflows/windows.yml?query=branch%3Amain" title="การทดสอบไลบรารีบน Windows ครั้งล่าสุด (สั่งรันด้วยตนเอง)"><img src="https://github.com/YuriPlanet/vera20k/actions/workflows/windows.yml/badge.svg?branch=main" alt="การทดสอบไลบรารีบน Windows" height="20" align="middle"></a>
  <a href="https://discord.gg/kmjRUn5m5F"><img src="https://img.shields.io/badge/Discord-Join-5865F2?style=flat&amp;logo=discord&amp;logoColor=white" alt="เข้าร่วม Discord" height="20" align="middle"></a>
</p>

# VERA20k

VERA20k คือการเขียนเอนจินเกมต้นฉบับ `gamemd.exe` ขึ้นใหม่ โดยใช้ไฟล์จากเกมต้นฉบับ
คุณจึงต้องมี Red Alert 2: Yuri's Revenge ของตัวเอง

VERA20k สร้างโดยเกมเมอร์ เพื่อเกมเมอร์ และเกมเมอร์คือผู้ตัดสินใจขั้นสุดท้ายว่าโครงการจะไปในทิศทางใด

<img src="docs/images/vera20k-screenshots.png" alt="หน้าตั้งค่าการต่อสู้และภาพขณะเล่น VERA20k" width="100%">

## เป้าหมายของโครงการ

1. คงรูปแบบการเล่น ภาพ และบรรยากาศของ Red Alert 2: Yuri's Revenge ต้นฉบับไว้
2. รองรับการต่อสู้ที่ใหญ่ขึ้น: สูงสุด **30 ผู้เล่น** และ **20,000 ยูนิต** บนแผนที่ขนาดใหญ่ขึ้น
3. เพิ่มฟีเจอร์ RTS ที่เป็นที่รู้จัก ทั้งเก่าและใหม่ รวมถึงฟีเจอร์ที่ไม่เคยมีมาก่อน
4. ไคลเอนต์ผู้เล่นหลายคนในตัว

## สถานะปัจจุบัน

**อยู่ในช่วงกลางของการพัฒนา** สามารถเล่นการต่อสู้ภายในเครื่องบน Windows กับ AI พื้นฐานได้แล้ว
มีแผนที่จากเกมต้นฉบับและแผนที่สุ่ม เมนู การสร้างฐาน การเก็บทรัพยากร การต่อสู้ และการบันทึก/โหลดเกมแล้ว
แต่ยังมีอีกมากที่ต้องแก้ไขและทำให้เสร็จ

ยังไม่มีโหมดผู้เล่นหลายคน แคมเปญ และ AI แบบเกมต้นฉบับ เครื่องบิน การควบคุมจิตใจ สะพาน
รวมถึงอาวุธและเอฟเฟกต์หลายอย่างยังต้องพัฒนาต่อ เรายังไม่ได้สาธิตการต่อสู้ที่มีผู้เล่น 30 คนและยูนิต 20,000 ตัว

## บิลด์และรัน

ต้องมี [Rust](https://rustup.rs/) เวอร์ชันเสถียรล่าสุด GPU ที่รองรับ Vulkan, DirectX 12 หรือ Metal และติดตั้งเกมไว้แล้ว
ใช้ Yuri's Revenge 1.001 รุ่นใดก็ได้ บน macOS ให้คัดลอกโฟลเดอร์เกมมาจากเครื่องที่ติดตั้งเกมบน Windows
บน Debian และ Ubuntu ต้องติดตั้ง `libasound2-dev` และ `pkg-config` เพิ่มด้วย มีการเล่น VERA20k บน Windows, Linux และ macOS แล้ว

```sh
git clone https://github.com/YuriPlanet/vera20k.git
cd vera20k
cp config.toml.example config.toml
# ก่อนรัน ให้แก้ไข config.toml และตั้งค่า ra2_dir เป็นโฟลเดอร์เกมของคุณ:
cargo run --release --bin vera20k
```

ให้เขียน `ra2_dir` ด้วยเครื่องหมายทับ (/) เช่น `C:/Games/RA2` ใช้ `--release` เพื่อเล่นเกม เพราะบิลด์แบบ debug ช้าเกินไป
ไฟล์ล็อกอยู่ที่ `logs/ra2.log`

## วิธีทำงานของเรา

โค้ดส่วนใหญ่เขียนโดยเอเจนต์ AI ซึ่งใช้ Ghidra ศึกษาเอนจินต้นฉบับ
แล้วนำพฤติกรรมของมันมาเขียนใน Rust และตรวจสอบด้วย[เครื่องมือเปรียบเทียบ](tools/native_oracle.md)
และการทดลองเล่น เอเจนต์ทำตาม [AGENTS.md](AGENTS.md) ส่วนผู้ร่วมพัฒนาทำตามกฎด้านล่าง

## ร่วมพัฒนา

ยินดีรับความช่วยเหลือ คุณจะเขียนโค้ด รีแฟกเตอร์เอนจิน ทดสอบเกม เสนอไอเดีย หรือเล่นเทียบกับเกมต้นฉบับ
แล้วบอกเราว่าตรงไหนรู้สึกผิดไปก็ได้ เปิด PR มาได้เลย ที่เหลือเราจัดการเอง ถ้าเป็นงานใหญ่ ให้ถามก่อนใน issue
หรือใน [Discord](https://discord.gg/kmjRUn5m5F)

โค้ดกลไกเกมอยู่ใน `src/sim/` การเรนเดอร์อยู่ใน `src/render/` ส่วนเมนูและการรับอินพุตอยู่ใน `src/app/`, `src/ui/`
และ `src/sidebar/` คุณไม่จำเป็นต้องใช้เครื่องมือ Python ใน `tools/`
[ภาพรวมสถาปัตยกรรม](https://yuriplanet.github.io/vera20k/th/) อธิบายว่าส่วนต่าง ๆ ของเอนจินทำงานร่วมกันอย่างไร
รันการทดสอบด้วย `cargo test -p vera20k --lib` การทดสอบที่ต้องใช้ไฟล์ INI ของเกมจะถูกข้ามไปแต่ยังนับว่าผ่าน
จนกว่าคุณจะรัน `cargo run --bin extract-ini [โฟลเดอร์เกม]`

1. **ยึดเกมต้นฉบับเป็นหลัก** ระบุฟังก์ชันต้นฉบับไว้ในคอมเมนต์เหมือนโค้ดรอบ ๆ
   (`/// MissionClass::Mission_Dispatch @ 0x005B3060`) หรือเขียนใน PR ว่าพฤติกรรมนั้นมาจากไหน อย่าเดา
2. **สถานะแต่ละส่วนมีเจ้าของเพียงหนึ่งเดียว** ขยายเจ้าของที่มีอยู่ และลบเส้นทางเก่าที่คุณแทนที่
3. **การจำลองต้องให้ผลแน่นอน (deterministic)** อินพุตเดียวกันต้องได้ผลลัพธ์เดียวกันบนทุกระบบปฏิบัติการและทุก CPU
   ใช้ `SimFixed` ใน `src/sim/` และคงลำดับการสุ่มและเอฟเฟกต์ภายในเฟรมเดียวกันให้ตรงกับเกมต้นฉบับ
4. **เพิ่มการทดสอบ** เมื่อเปลี่ยนกลไกการเล่น
5. **ห้ามคอมมิตไฟล์เกม** (`.mix`, INI, ภาพ, เสียง, วิดีโอ, `.exe`) หรือสิ่งใดก็ตามใน `ini/`

ผลงานที่ร่วมพัฒนาใช้สัญญาอนุญาต GPLv3 เช่นเดียวกับส่วนอื่นของโครงการ และไม่ต้องลงนาม CLA

## เครดิตและข้อกฎหมาย

ขอบคุณ OpenRA, XCC Mixer, วิกิ ModEnc, Project Perfect Mod, EA ที่เผยแพร่ซอร์สโค้ดของ
Command & Conquer และ Red Alert ภายใต้ GPL, World-Altering Editor, Final Alert, YRpp, Ares, Phobos
และอีกหลายโครงการ

เผยแพร่ภายใต้สัญญาอนุญาต [GPLv3](LICENSE-GPL) ที่เก็บโค้ดนี้ไม่มีไฟล์เกม
Command & Conquer และ Red Alert เป็นเครื่องหมายการค้าของ Electronic Arts Inc.
ภาพหน้าจอแสดงงานภาพจากเกมที่เป็นของ Electronic Arts
VERA20k ไม่มีความเกี่ยวข้องกับ Electronic Arts และไม่ได้รับการรับรองจาก Electronic Arts

แปลจาก [README.md](README.md) โปรดปรับปรุงให้ตรงกับฉบับภาษาอังกฤษเสมอ
