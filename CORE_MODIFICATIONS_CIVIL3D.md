# وثيقة التعديلات البرمجية على نواة النظام (OpenCADStudio Core Modifications)
## ترقية النواة لدعم ميزات الهندسة المدنية والأسطح ثلاثية الأبعاد (Civil 3D Parity)

تاريخ التوثيق: أكتوبر 2026  
المستودع الأساسي: `OpenCADStudio-2026.41`  
المسار: `C:\Users\j\Videos\OpenCADStudio-2026.41\OpenCADStudio-2026.41`  
لغة البرمجة: **Rust** (بواسطة إطار العمل الرسومي **Iced** ومحرك الرسم ثلاثي الأبعاد **wgpu**)

---

## 1. الفهرس والملفات المضافة والمعدلة في الـ Core

| المسار البرمجي (File Path) | نوع التعديل | الغرض التقني والوظيفي |
| :--- | :---: | :--- |
| `src/app/commands/civil_surface.rs` | **ملف جديد** (73 KB) | المحرك الرياضي والهندسي للأسطح: بنك خوارزميات TIN، حذف المثلثات، تبديل الحواف، رفع/خفض المناسيب، حساب حجوم القطع والردم، وتصدير LandXML. |
| `src/ui/window/surface_properties.rs` | **ملف جديد** (19 KB) | نافذة خصائص السطح (Civil 3D Surface Properties Dialog) بـ 4 تبويبات تفاعلية (Information, Definition, Analysis, Statistics). |
| `src/ui/window/parcel_layout_tools.rs` | **ملف جديد** (20 KB) | أدوات تخطيط وتجزئة الحصص والأراضي (Parcel Sizing & Layout Tools) بخوارزميات Slide Line و Swing Line. |
| `src/ui/window/parcel_properties.rs` | **ملف جديد** (22 KB) | نافذة فحص وتعديل خصائص الحصص العقارية، الترقيم التلقائي، ومطابقة المساحات والواجهات. |
| `src/modules/view/toolspace.rs` | **ملف جديد** (1 KB) | تعريف أداة شريط الريبون `Toolspace\n(Prospector)` مع الأيقونة والحدث المرتبط بها. |
| `src/ui/window/browser.rs` | **معدل** | تحويل لوحة المستعرض الجانبية إلى شجرة **Prospector (Toolspace)** مع عناصر تحكم سريعة للأسطح وتعديلات الـ TIN المباشرة. |
| `src/ui/dock.rs` | **معدل** | ترقية هوية اللوحة إلى `Prospector`، ضبط العرض الافتراضي إلى `260.0px`، ودعم تكامل الإرساء على الحافة اليمنى. |
| `src/app/mod.rs` | **معدل** | تعديل قيمة الإقلاع الافتراضية `show_browser: true` لضمان ظهور لوحة الـ Prospector تلقائياً عند فتح البرنامج. |
| `src/app/commands/layerprops.rs` | **معدل** | برمجة معالجة الأوامر `PROSPECTOR` و `TOOLSPACE` و `BROWSER` لتثبيت وتوسيع لوحة شجرة المشروع. |
| `src/app/commands/mod.rs` | **معدل** | تسجيل الأوامر الجديدة `PROSPECTOR` و `TOOLSPACE` في جدول أوامر الـ CAD المعتمدة. |
| `src/modules/view/mod.rs` | **معدل** | إضافة زر `Toolspace (Prospector)` ضمن مجموعة `لوحات التحكم` (Palettes) في شريط الأدوات (Ribbon). |
| `tests/civil_dynamic_parcel_reactor.rs` | **ملف جديد** (8 KB) | اختبارات الوحدة والتكامل للتحقق من الاستجابة الديناميكية وتحديث الحدود والحصص. |

---

## 2. الشرح التفصيلي للمكونات البرمجية

### 2.1. محرك الأسطح وعمليات الـ TIN (`src/app/commands/civil_surface.rs`)

تم بناء محرك هندسي متكامل للتعامل مع شبكات المثلثات غير المنتظمة (Triangulated Irregular Network - TIN) وشبكات `PolyfaceMesh`:

1. **بنية البيانات الأساسية (`TinTriangle` & `TinSurfaceData`)**:
   - `TinTriangle`: تمثيل ثلاثي الرؤوس مع دوال سريعة لحساب المساحة السطحية في المسقط الأفقي `area_2d()` والمساحة الحقيقية ثلاثية الأبعاد في الفراغ `area_3d()` باستخدام الضرب المتجهي (Cross Product).
   - اختبار احتواء النقطة داخلياً بنظام الإحداثيات المثلثية (Barycentric Coordinates) عبر دالة `contains_point_2d(px, py)`.
   - `TinSurfaceData`: هيكل يحمل بيانات السطح الكاملة، بما في ذلك المقبض الفريد (`Handle`)، الطبقة، الحدود الخارجية والداخلية، الإحصائيات، وسجل العمليات.

2. **أدوات التعديل التفاعلية بالنقر (Interactive TIN Editing Operations)**:
   - **حذف المثلثات تفاعلياً (`SurfaceDeleteTriangleCommand`)**:
     - تطبيق سمة `CadCommand`.
     - عند استدعاء الأمر، يظهر موجه في سطر الأوامر يطلب من المستخدم:
       `"Select triangle to delete (click surface in viewport) or [Undo/eXit]:"`
     - يلتقط إحداثيات النقر ثلاثية الأبعاد عبر `pick_point`، ويحدد المثلث المتقاطع بدقة في كائن `PolyfaceMesh` أو `Mesh`، ويقوم بإزالته فورياً وتحديث الرسم وحفظ لقطة للتراجع (Undo Snapshot).
   - **تبديل الحواف تفاعلياً (`SurfaceSwapEdgeCommand`)**:
     - خوارزمية `swap_edge_in_mesh` و `swap_edge_in_surface`: تفحص الحافة المشتركة بين مثلثين متجاورين، وتتأكد من أن الشكل الرباعي محدب (Convex Quad)، ثم تعيد توجيه الوتر لتحسين جودة شبكة الـ Delaunay دون تغيير الرؤوس.
   - **تعديل المناسيب (`raise_lower_surface`)**:
     - إمكانية زيادة أو خفض منسوب كافة رؤوس السطح بقيمة محددة (مثل `+1.0m` أو `-1.0m`) مع تحديث مصفوفة الإحداثيات وحساب الارتفاعات الإجمالية.

3. **حساب حجوم القطع والردم (`calculate_volumes`)**:
   - مقارنة السطح الأساسي (Base Surface مثل الأرض الطبيعية EG) بالسطح المقارن (Comparison Surface مثل حفرة التسوية Pit).
   - خوارزمية تقاطع المنشورات المثلثية (Prismatic Method) لحساب حجم الحفر (Cut Volume)، حجم الردم (Fill Volume)، والحجم الصافي (Net Volume).
   - دالة `format_volume_report` لتوليد تقرير هندسي منظم يمكن طباعته في موجه الأوامر.

4. **تصدير واستيراد LandXML (`export_to_landxml`)**:
   - توليد ملف XML متوافق بنسبة 100% مع معيار `LandXML 1.2` لتبادل الأسطح والنقاط والمثلثات مع برامج Civil 3D و Infraworks.

5. **تبديل أنماط السطح ديناميكياً (`apply_surface_style`)**:
   - إمكانية التبديل الفوري بين عرض المثلثات فقط (`Triangles`)، خطوط الكنتور (`Contours`)، أو كلاهما (`Both`) عن طريق إدارة طبقات الـ CAD وتوليد خطوط الكنتور الفورية بفواصل زمنية قابلة للضبط.

---

### 2.2. نافذة خصائص السطح (`src/ui/window/surface_properties.rs`)

نافذة حوارية احترافية مبنية بـ Iced UI تحاكي نافذة Civil 3D Surface Properties بدقة عالية وتتألف من 4 تبويبات رئيسية:

1. **تبويب المعلومات (Information Tab)**:
   - اسم السطح والوصف الهندسي.
   - القائمة المنسدلة لاختيار نمط السطح (`Surface Style`) مثل:
     - `Contours 1m and 5m (Design)`
     - `Contours 2m and 10m (Background)`
     - `Triangles Only`
     - `Elevation Banding (1m intervals)`
     - `Slope Analysis (6 ranges)`
   - خيار تثبيت وقفل الكائن (`Object Locked`).

2. **تبويب التعريف وسجل العمليات (Definition Tab)**:
   - شجرة العمليات السابقة (Operations Tree) مع مربعات اختيار تفاعلية لإعادة بناء السطح:
     - استبعاد الارتفاعات الأقل من حد معين (`Exclude Elevations Less Than`).
     - استبعاد الارتفاعات الأعلى من حد معين (`Exclude Elevations Greater Than`).
     - تحديد أقصى طول مسموح به لحافة المثلث (`Maximum Triangle Length`).

3. **تبويب التحليل الهندسي (Analysis Tab)**:
   - دعم أنواع التحليل: الكنتور (Contours)، المناسيب (Elevations)، الميول (Slopes)، والاتجاهات (Aspects).
   - إمكانية تحديد عدد النطاقات (`Ranges Count`) وتوليد جدول تدرج لوني تلقائي للسطح.

4. **تبويب الإحصائيات (Statistics Tab)**:
   - تقرير حي يعرض:
     - إجمالي عدد الرؤوس (Vertices Count) وعدد الوجوه/المثلثات (Triangles Count).
     - أدنى منسوب (`Min Elevation`)، أقصى منسوب (`Max Elevation`)، ومتوسط المنسوب (`Mean Elevation`).
     - المساحة المسطحة ثنائية الأبعاد (`2D Area m²`) والمساحة السطحية الحقيقية ثلاثية الأبعاد (`3D Surface Area m²`).

---

### 2.3. شجرة الـ Prospector ونظام الإرساء (`src/ui/window/browser.rs` & `src/ui/dock.rs`)

1. **إعادة هيكلة شجرة المشروع (Project Tree)**:
   - تقديم كائنات Civil 3D لتكون في قمة الشجرة فوق المستويات الميكانيكية الأصلية:
     - **`Prospector - Surfaces (n)`**
     - **`Parcels / Subdivisions`**
     - **`Feature Lines / Breaklines`**
     - **`Survey Points`**
   - زر كبير في رأس المجموعة: `[➕ Create Surface from Points]`.
   - زر علوي: `[📊 Calculate Cut / Fill Volume]`.

2. **بطاقات الأسطح التفاعلية (Surface Cards)**:
   - كل سطح في الشجرة يظهر في بطاقة رمادية مخصصة مع معلومات المنسوب والمساحة:
     `EG [37]: Z: 99.2..108.5m | Area: 67612m² | 601 tris`
   - **أزرار سريعة داخل البطاقة**:
     - `[🔍 Zoom]`: يقرب كاميرا العرض 3D مباشرة إلى حدود السطح.
     - `[⚙ Props]`: يفتح نافذة خصائص السطح (Surface Properties Dialog).
     - `[📊 Stats]`: يطبع الإحصائيات الشاملة في سطر الأوامر.
     - `[📥 LandXML]`: يُصدر السطح إلى ملف LandXML.
     - `[🔺 Triangles]` / `[〰 Contours]` / `[ Both ]`: للتحكم في النمط المرئي.
     - `[✂ Del Tri]`: يفعل وضع حذف المثلث بالنقر اليدوي.
     - `[🔄 Swap]`: يفعل وضع تبديل الحواف بالنقر اليدوي.
     - `[⬆ +1m]` / `[⬇ -1m]`: لتعديل منسوب السطح رأسياً فوراً.

3. **ترقية نظام الإرساء (Dock System)**:
   - تغيير تسمية المعرف `PanelId::Browser` في `src/ui/dock.rs` إلى `"Prospector"`.
   - ضبط العرض التلقائي إلى `260.0px` لتوفير مساحة كافية لقراءة شجرة العمليات والبيانات الهندسية دون حجب شاشة الرسم.
   - في `src/app/mod.rs` (السطر 4359)، تم ضبط `show_browser: true` افتراضياً ليفتح الـ Prospector تلقائياً عند تشغيل البرنامج.

---

### 2.4. تكامل موجه الأوامر وشريط الأدوات (Commands & Ribbon)

1. **موجه الأوامر (`src/app/commands/layerprops.rs` & `src/app/commands/mod.rs`)**:
   - تم تسجيل ومعالجة الأوامر:
     ```rust
     "BROWSER" | "PROSPECTOR" | "TOOLSPACE" => {
         let id = crate::ui::dock::PanelId::Browser;
         // Toggle, dock to right edge, and expand panel
     }
     ```
   - عند كتابة أي من هذه الأوامر في شريط الأوامر السفلي، يقوم النظام بتثبيت وتوسيع لوحة الـ Prospector على يمين الشاشة فوراً مع إعطاء رسالة تأكيد خضراء.

2. **أداة شريط الريبون (`src/modules/view/toolspace.rs` & `src/modules/view/mod.rs`)**:
   - تم إنشاء وحدة برمجية جديدة `src/modules/view/toolspace.rs` تُعرف الأداة:
     ```rust
     pub fn tool() -> ToolDef {
         ToolDef {
             id: "TOOLSPACE",
             label: "Toolspace\n(Prospector)",
             icon: IconKind::Glyph("\u{25A4}"),
             event: ModuleEvent::Command("TOOLSPACE".to_string()),
         }
     }
     ```
   - تم ربط الأداة في شريط الأدوات ضمن تبويب `عرض` (`View`) داخل مجموعة `لوحات التحكم` (`Palettes`) بجوار لوحة الخصائص (`Properties`)، مما يتيح إظهار وإخفاء لوحة الـ Prospector بنقرة ماوس واحدة.

---

### 2.5. نظام الحصص والمخططات الديناميكي (Dynamic Parcel Reactor)

1. **`src/ui/window/parcel_layout_tools.rs`**:
   - نافذة تفاعلية تتيح تحديد المعايير التخطيطية للأراضي:
     - المساحة الدنيا للقطعة (`Minimum Area`).
     - الواجهة الأمامية الدنيا (`Minimum Frontage`).
     - زاوية الميل بالنسبة للشارع أو خط الأساس (`Angle relative to alignment`).
     - تنفيذ التجزئة التلقائية بخطوط متوازية أو متجهة نحو المركز.

2. **`src/ui/window/parcel_properties.rs`**:
   - عرض المعرفات الهندسية لكل قطعة أرض، المحيط، المساحة بالمتر المربع والهكتار والقدم المربع، والطبقة المرتبطة بها.

3. **`tests/civil_dynamic_parcel_reactor.rs`**:
   - اختبار تكامل يثبت قدرة النواة على إعادة حساب مساحات وأرقام الحصص العقارية لحظياً عند تحريك أي حد أو نقطة مشتركة في الرسم.

---

## 3. آلية البناء والتحقق (Build & Verification)

لإعادة بناء البرنامج بعد هذه التعديلات:

```powershell
# 1. إيقاف أي عمليات قديمة لـ OpenCADStudio لمنع قفل الملف التنفيذي
Stop-Process -Name "OpenCADStudio*" -Force -ErrorAction SilentlyContinue

# 2. بناء المشروع بنمط التطوير (Debug Profile)
cd "C:\Users\j\Videos\OpenCADStudio-2026.41\OpenCADStudio-2026.41"
cargo build

# 3. أو بناء نسخة الإصدار النهائية عالية الأداء (Release Profile)
cargo build --release
```

- الملف التنفيذي الناتج يقع في:
  - نمط التطوير: `target\debug\OpenCADStudio.exe`
  - نمط الإصدار: `target\release\OpenCADStudio.exe`

---

## 4. الخلاصة

بفضل هذه التعديلات على نواة OpenCADStudio Core:
1. أصبح البرنامج يمتلك شجرة **Prospector / Toolspace** أصلية مدمجة في واجهته كلوحة يمنى ثابتة ونشطة افتراضياً.
2. تتوفر كافة أدوات تعديل أسطح الـ TIN (حذف المثلثات تفاعلياً، تبديل الحواف، رفع/خفض المناسيب، وحساب الكميات) بأسلوب نقر مباشر على لوحة الرسم 3D دون الحاجة لكتابة أوامر طرفية.
3. أصبح البرنامج متوافقاً مع سير عمل أوتوكاد و Civil 3D القياسي سواء عبر الماوس أو موجه الأوامر أو ملفات تبادل LandXML.
