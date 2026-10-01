# Artaveo Dental

## Master Development Roadmap — V1

### تعریف محصول

Artaveo Dental یک **Windows Desktop Application** برای مدیریت کامل کلینیک دندان‌پزشکی است؛ با قابلیت کارکرد آفلاین، دیتابیس محلی، License مرکزی، Backup چندلایه، رابط مدرن و چندزبانه، و معماری Single-Codebase که بتواند برای کلینیک‌های متعدد توزیع شود.

### معماری پایه‌ای که تمام فازها باید از آن پیروی کنند

```text
Windows Desktop App
        │
        ├── Tauri
        ├── React + TypeScript
        ├── Local Application Services
        └── SQLite
                │
                └── Clinic Operational Data

Internet — Optional for daily operations
        │
        └── Central Services
                ├── License
                ├── Activation
                ├── Installation Tracking
                ├── Security Events
                ├── Cloud Backup
                └── App Update Metadata
```

اصل مهم:

**Cloud نباید Single Point of Failure برای عملیات روزمره کلینیک باشد.**

قطع اینترنت نباید مانع مشاهده بیمار، ثبت درمان، نوبت، پرداخت یا صدور رسید شود.

معماری باید از ابتدا برای Multi-Clinic طراحی شود، ولی هر کلینیک در محیط Local داده خودش را جدا نگه می‌دارد.

---

# PHASE 1 — Product Foundation, Architecture & Engineering Core

هدف این فاز ساخت «اسکلت واقعی محصول» است، نه UI نهایی.

### 1.1 Desktop Foundation

راه‌اندازی:

* Tauri 2
* React
* TypeScript
* Vite
* Windows Build
* Application Installer
* Application Versioning
* Environment Configuration
* Dev / Test / Production configuration

خروجی نهایی:

```text
ArtaveoDental.exe
```

که روی Windows نصب و اجرا شود.

### 1.2 Local Data Architecture

SQLite به‌عنوان Operational Database.

اصول:

* Foreign Keys فعال
* WAL Mode
* Transactions
* Prepared Queries
* Index Strategy
* Database migrations
* Schema versioning
* Seed data
* Integrity checks

SQLite در حالت STRICT نیز امکان کنترل سخت‌گیرانه‌تر نوع داده را دارد؛ در طراحی نهایی هر جا مفید باشد از آن استفاده می‌کنیم.

### 1.3 Reference Data Architecture

برای داده‌هایی مثل:

```text
Gender
Province
District
City
Blood Group
Marital Status
Payment Method
Appointment Status
Treatment Status
Tooth Condition
...
```

تا جای ممکن ذخیره متن تکراری نخواهیم داشت.

مثلاً:

```text
patients.gender_id = 1
```

و جدول مرجع:

```text
gender
1 = Male
2 = Female
```

برای Province و District و City نیز:

```text
province_id
district_id
city_id
```

با Foreign Key.

اما نام‌های نمایشی در جداول Translation نگهداری می‌شوند:

```text
reference_translations

reference_id
language_id
label
```

بنابراین یک ID ثابت می‌تواند در فارسی، پشتو و انگلیسی نام متفاوت داشته باشد.

این دقیقاً مناسب چیزی است که گفتی.

برای مقادیر کوچک و کاملاً ثابت می‌توان از Enum هم استفاده کرد، ولی برای داده‌های قابل توسعه و چندزبانه مثل شهر و ولایت، Reference Table مناسب‌تر است؛ خود مستندات Supabase/Postgres هم Enum را بیشتر برای مقادیر کوچک و کم‌تغییر توصیه می‌کنند.

### 1.4 Identity & Security Foundation

ساخت:

* Clinic ID
* User ID
* Installation ID
* License ID
* Audit ID
* UUID/ID strategy
* Local authentication
* Password hashing
* Session management
* Permission model

### 1.5 Design System Foundation

طراحی Design System مرکزی:

* Typography
* Spacing
* Radius
* Shadows
* Glass surfaces
* Cards
* Tables
* Inputs
* Buttons
* Dialogs
* Toasts
* Modals
* Empty states
* Loading states
* Error states

سه زبان از ابتدا:

```text
فارسی ← Default
English
پښتو
```

دو Theme:

```text
Light
Dark
```

و امکان تنظیم:

* Clinic Logo
* Clinic Name
* Primary Color
* Secondary Color
* Accent
* Receipt Branding
* Application Branding

---

# PHASE 2 — Application Shell, Localization & Clinic Identity

در این فاز خود برنامه از نظر UX شکل محصول واقعی می‌گیرد.

### 2.1 Application Shell

ساخت:

* Sidebar
* Top Bar
* Global Search
* Notifications
* User Menu
* Clinic Identity
* Breadcrumbs
* Page Layout
* Modal System
* Dialog System
* Command/Search navigation

### 2.2 Multilingual Engine

این بخش فقط ترجمه منو نیست.

باید:

* RTL/LTR واقعی
* Mixed Persian + English
* Mixed Pashto + English
* Numeric rendering
* Date rendering
* Currency rendering
* Print rendering
* PDF rendering

را مدیریت کند.

مثلاً:

```text
بیمار: John Smith
درمان: Root Canal Treatment
مبلغ: 2,500 AFN
```

نباید Layout خراب شود.

### 2.3 Data Language ≠ Interface Language

زبان برنامه:

```text
Persian
```

ولی داده بیمار:

```text
Ahmad Khan
```

یا:

```text
احمد خان
```

یا:

```text
Ahmad خان
```

همه باید معتبر باشند.

اطلاعاتی که خود کاربر وارد می‌کند **ترجمه خودکار نمی‌شود**.

برای موارد تحت کنترل سیستم مثل:

* Service Names
* Treatment Names
* Statuses
* Reference Data
* Departments

از Localization استفاده می‌شود.

### 2.4 Clinic Setup Wizard

اولین اجرا:

```text
Clinic Name
Clinic Logo
Province
District
City
Address
Phone
Currency
Language
Theme
Working Hours
Timezone
```

واحد پول پیش‌فرض:

```text
AFN
```

ولی ساختار برای ارزهای دیگر باز باشد.

---

# PHASE 3 — Patients, People & Complete Medical Records

این فاز هسته اصلی سیستم است.

### 3.1 Patient Management

هر بیمار دارای:

* Patient ID
* Patient Number
* Full Name
* Preferred Language
* Gender ID
* Date of Birth
* Phone
* Secondary Phone
* Province ID
* District ID
* City ID
* Address
* Emergency Contact
* Notes
* Registration Date
* Status

باشد.

### 3.2 Patient Search

جستجوی سریع بر اساس:

* نام
* شماره بیمار
* شماره تماس
* کد
* بخشی از نام
* اطلاعات تکمیلی

با Indexهای مناسب.

### 3.3 Patient Profile

صفحه بیمار:

```text
Overview
Appointments
Clinical History
Dental Chart
Treatments
Treatment Plans
Invoices
Payments
Documents
Images
Notes
Audit History
```

### 3.4 Medical History

ثبت:

* Allergies
* Current Medications
* Previous Conditions
* Medical History
* Dental History
* Previous Surgeries
* Relevant Notes

### 3.5 Attachments

امکان اضافه‌کردن:

* X-Ray
* Dental Photos
* Documents
* Scans
* Consent forms

با Metadata مناسب.

### 3.6 Patient Timeline

تمام اتفاقات بیمار در Timeline:

```text
Registered
Appointment
Examination
Diagnosis
Treatment
Payment
Document
Follow-up
```

---

# PHASE 4 — Dental Clinical Core

این فاز چیزی است که محصول را از یک CRM/Clinic Manager ساده به **Dental Management System واقعی** تبدیل می‌کند.

### 4.1 Dental Chart / Odontogram

پشتیبانی از دندان‌ها به‌شکل بصری.

حداقل:

* Permanent Teeth
* Primary/Deciduous Teeth
* Upper / Lower
* Left / Right
* Individual Tooth
* Tooth Surface

سیستم باید ساختار شماره‌گذاری استاندارد داشته باشد و در صورت نیاز بتواند Scheme مناسب را انتخاب کند.

### 4.2 Tooth States

برای هر دندان:

* Healthy
* Caries
* Filled
* Missing
* Extracted
* Crown
* Root Canal
* Implant
* Bridge
* Other configurable conditions

### 4.3 Tooth Surface

ثبت وضعیت روی سطوح مختلف:

```text
Mesial
Distal
Occlusal
Buccal
Lingual
```

در صورت نیاز، وضعیت‌های تخصصی‌تر در نسخه‌های بعدی اضافه می‌شوند.

### 4.4 Periodontal Chart

ثبت شاخص‌های پریودنتال در ساختار استاندارد قابل توسعه.

### 4.5 Clinical Notes

دندان‌پزشک بتواند برای هر Visit ثبت کند:

* Chief Complaint
* Examination
* Diagnosis
* Findings
* Notes
* Procedure
* Follow-up
* Recommendations

### 4.6 Treatment Plan

ساخت:

```text
Treatment Plan
   ├── Procedure
   ├── Tooth
   ├── Doctor
   ├── Estimated Cost
   ├── Priority
   ├── Status
   └── Notes
```

و وضعیت‌ها:

```text
Proposed
Accepted
In Progress
Completed
Cancelled
```

### 4.7 Treatment History

هر درمان با تاریخ، دندان، پزشک، هزینه و نتیجه در History بیمار ثبت شود.

---

# PHASE 5 — Appointments, Doctors & Daily Clinic Workflow

این فاز سیستم را از «پرونده پزشکی» به «سیستم عملیاتی روزانه» تبدیل می‌کند.

### 5.1 Appointment Engine

تقویم:

* Day
* Week
* Month

و Appointment شامل:

* Patient
* Doctor
* Chair/Room
* Date
* Start
* End
* Reason
* Status
* Notes

### 5.2 Appointment Status

مثلاً:

```text
Scheduled
Confirmed
Checked-in
In Treatment
Completed
Cancelled
No-show
Rescheduled
```

### 5.3 Doctor Schedule

برای هر دکتر:

* Working Days
* Working Hours
* Breaks
* Available Chairs
* Leave/Unavailable periods

### 5.4 Queue / Reception Workflow

مدل:

```text
Appointment
      ↓
Patient Arrived
      ↓
Waiting
      ↓
Doctor
      ↓
Treatment
      ↓
Checkout
      ↓
Payment
```

### 5.5 Follow-up

مثلاً:

```text
Next Visit
Follow-up Date
Recall
Treatment Reminder
```

### 5.6 Staff & Roles

Roleهای پایه:

```text
Owner
Administrator
Receptionist
Doctor
Accountant
Assistant
```

ولی Permissionها granular باشند.

مثلاً Receptionist بتواند Appointment بسازد اما به اطلاعات حساس مالی خاص یا تنظیمات سیستم دسترسی نداشته باشد.

---

# PHASE 6 — Billing, Payments, Receipts & Professional Printing

این فاز سیستم مالی کلینیک است.

### 6.1 Service Catalog

ساخت لیست خدمات:

```text
Consultation
Filling
Extraction
Root Canal
Crown
Implant
Cleaning
...
```

برای هر Service:

* Service ID
* Localized Name
* Default Price
* Category
* Active/Inactive

### 6.2 Treatment Pricing

قیمت واقعی می‌تواند در زمان Treatment یا Invoice Override شود.

### 6.3 Invoice

Invoice شامل:

* Patient
* Services
* Quantity
* Unit Price
* Discount
* Total
* Paid
* Balance
* Payment Status

### 6.4 Payments

پشتیبانی از:

* Cash
* Transfer
* Other configurable methods
* Partial Payment
* Installment
* Refund

### 6.5 Debt / Balance

سیستم باید همیشه بتواند نشان دهد:

```text
Total
Paid
Discount
Refunded
Remaining
```

### 6.6 Receipt Numbering

شماره یکتا:

```text
RC-2026-000001
RC-2026-000002
...
```

و هیچ Payment نباید با Reprint اشتباهاً دوباره ثبت شود.

### 6.7 Compact Receipt Engine

این بخش مطابق تصمیم جدید ما طراحی می‌شود.

**Receipt یک Document Page Size واقعی دارد.**

مثلاً:

```text
Thermal 58mm
Thermal 80mm
Compact A6
Custom Compact
```

در چاپ با پرینتر A4، رسید روی بخشی کوچک از کاغذ قرار می‌گیرد و فضای خالی عظیم مثل یک گزارش A4 ایجاد نمی‌کند.

مثلاً:

```text
┌────────────────────────────── A4 ──────────────────────────────┐
│                                                               │
│      ┌────────────── COMPACT RECEIPT ──────────────┐          │
│      │                                               │          │
│      │       KABUL SMILE DENTAL                    │          │
│      │       Receipt RC-000184                      │          │
│      │       Patient: Ahmad Khan                    │          │
│      │       Root Canal      2,500 AFN              │          │
│      │       Paid             1,500 AFN              │          │
│      │       Balance          1,000 AFN              │          │
│      │                                               │          │
│      └───────────────────────────────────────────────┘          │
│                                                               │
└───────────────────────────────────────────────────────────────┘
```

محل قرارگیری Receipt روی صفحه نیز قابل تنظیم خواهد بود.

### 6.8 PDF

خیلی مهم:

وقتی کاربر:

```text
Save as PDF
```

می‌زند، PDF **نباید یک A4 خالی با رسید کوچک وسط آن باشد**.

مثلاً PDF خودش:

```text
Page Size = A6
```

یا:

```text
Page Size = 80mm × dynamic height
```

خواهد داشت.

بنابراین PDF همان سند کوچک و حرفه‌ای است که برای نگهداری، ارسال یا چاپ دوباره مناسب است.

### 6.9 Full Documents

برای مواردی مثل:

* Full Invoice
* Financial Report
* Patient Report
* Treatment Plan

حالت A4 همچنان وجود خواهد داشت.

یعنی:

```text
Receipt ≠ A4 Report
```

این دو Template مستقل دارند.

---

# PHASE 7 — Inventory, Suppliers, Lab & Operational Management

برای اینکه سیستم واقعاً در یک کلینیک کامل قابل استفاده باشد، فقط بیمار و پول کافی نیست.

### 7.1 Inventory

ثبت:

* Items
* Categories
* Units
* Stock
* Minimum Stock
* Expiry Date
* Batch/Lot where applicable

### 7.2 Stock Operations

* Purchase
* Receive
* Consume
* Adjustment
* Return
* Transfer

### 7.3 Low Stock

مثلاً:

```text
Latex Gloves
⚠ Low Stock
```

### 7.4 Expiry Monitoring

برای اقلام تاریخ‌دار:

```text
Expired
Expiring Soon
Valid
```

### 7.5 Suppliers

* Supplier
* Contact
* Purchases
* Outstanding Balance
* History

### 7.6 Dental Lab

برای مواردی مثل:

* Crown
* Bridge
* Denture
* Other Lab Work

ثبت:

```text
Lab Case
Patient
Doctor
Sent Date
Expected Date
Received Date
Lab
Status
Cost
Notes
```

---

# PHASE 8 — Reports, Dashboard, Notifications, Audit & Management

در این فاز سیستم به یک ابزار مدیریتی واقعی تبدیل می‌شود.

### 8.1 Dashboard

نمایش:

```text
Today's Patients
Today's Appointments
Completed Treatments
Today's Revenue
Outstanding Balance
Upcoming Appointments
Low Stock
Pending Lab Cases
Backup Status
License Status
```

### 8.2 Financial Reports

* Daily Revenue
* Monthly Revenue
* Payments
* Outstanding Balances
* Discounts
* Refunds
* Doctor Revenue
* Service Revenue

### 8.3 Clinical Reports

* Patients
* Treatments
* Procedures
* Tooth Conditions
* Doctor Activity
* Follow-ups

### 8.4 Inventory Reports

* Stock
* Low Stock
* Purchase
* Consumption
* Expiry

### 8.5 Notification Center

Notificationهای داخل برنامه و Windows Notification برای Eventهای مهم:

```text
Appointment Reminder
Low Stock
Backup Failed
USB Missing
License Warning
Update Available
```

### 8.6 Audit Log

عملیات مهم ثبت شوند:

```text
Who
What
When
Which Record
Old Value
New Value
```

برای موارد حساس مثل:

* Delete
* Edit Patient
* Payment Modification
* Refund
* Discount
* Permission Change
* License Event

### 8.7 Global Search

جستجوی سریع در Entityهای اصلی بدون Queryهای سنگین و بدون Full-table scanهای غیرضروری.

---

# PHASE 9 — Offline Reliability, Backup, License & Security

این فاز قلب قابلیت اطمینان محصول است.

### 9.1 Offline Guarantee

این قابلیت‌ها بدون اینترنت باید کار کنند:

```text
Login
Patients
Appointments
Clinical Data
Dental Chart
Treatments
Invoices
Payments
Reports
Printing
Backup
```

### 9.2 License

Flow:

```text
Install
   ↓
First Activation
   ↓
Trial / Paid License
   ↓
Signed Local License State
   ↓
Periodic Verification
```

قواعد:

* Trial فقط یک بار
* Activation اولیه آنلاین
* Verification دوره‌ای
* Grace Period
* Expiration
* Manual Revocation
* Installation ID
* Installation Limit
* Unknown Installation Detection
* Read-only after defined failure state
* Local clock tamper detection
* Renewal from previous expiry, not from re-purchase date

مثال:

```text
Previous Expiry:
2027-01-01

Customer renews:
2027-04-01

New License Effective Start:
2027-01-01
```

نه 2027-04-01.

### 9.3 Backup Manager

Schedule قابل تنظیم:

```text
Default:
19:00
```

برای هر کلینیک مستقل.

پس از Schedule:

```text
Create Backup
      ↓
Verify
      ↓
Encrypt
      ↓
Local Backup
      ↓
Trusted USB
      ↓
Cloud Queue
```

### 9.4 USB Backup

تشخیص Device با Identity واقعی، نه فقط Drive Letter.

اولین اتصال:

```text
New Backup Device Detected
[Use for Backup]
[Ignore]
```

بعد از تأیید:

```text
Trusted
```

اتصال‌های بعدی:

```text
Detect
→ Verify Identity
→ Automatic Backup
```

### 9.5 Missing USB

اگر Backup Device مورد انتظار موجود نباشد:

```text
19:00 → Warning
19:30 → Warning
20:00 → Warning
...
```

Interval قابل تنظیم خواهد بود.

و در صورت وصل‌شدن USB:

```text
USB Detected
→ Pending Backup
→ Automatic Transfer
```

### 9.6 Backup Retention

پیشنهاد V1:

```text
Daily:
14 days

Weekly:
4 copies

Monthly:
3 copies
```

Cleanup خودکار باشد.

### 9.7 Offline + Cloud

اگر Internet نبود:

```text
Local Backup ✅
USB Backup ✅/Pending
Cloud Backup Pending
```

با بازگشت اینترنت:

```text
Pending Queue
→ Upload
→ Verify
→ Mark Complete
```

### 9.8 Restore

باید بتوان از Backup:

```text
Preview
Validate
Restore
```

استفاده کرد.

Restore نباید بدون Validation فایل Backup انجام شود.

### 9.9 Security

* Encrypted sensitive local data / approved encryption strategy
* Encrypted Backup
* Password hashing
* Protected application configuration
* Secure communication with License API
* No service-role/secret credentials inside EXE
* Signed application builds
* Secure migration process

Supabase Edge Functions برای Endpointهای مرکزی License و منطق server-side مناسب‌اند و می‌توانند با Auth و Postgres/Storage تعامل کنند.

---

# PHASE 10 — Production Hardening, Installer, Update & Release

این فاز تفاوت بین «پروژه‌ای که روی کامپیوتر من کار می‌کند» و «محصولی که می‌شود فروخت» است.

### 10.1 Installation

ساخت Installer حرفه‌ای:

```text
Artaveo Dental Setup.exe
```

با:

* Install
* Uninstall
* Repair
* Upgrade
* Data preservation
* Migration

### 10.2 First Launch Experience

```text
Welcome
   ↓
Clinic Setup
   ↓
Language
   ↓
Theme
   ↓
Admin User
   ↓
Trial / Activation
   ↓
Backup Setup
   ↓
Finished
```

### 10.3 Automatic Updates

برنامه هنگام Online شدن:

```text
Current Version
vs
Latest Version
```

را بررسی کند.

Update باید:

* Signed
* Versioned
* Safe
* Rollback-aware

باشد.

### 10.4 Crash & Recovery

اگر برنامه ناگهان بسته شد:

* Database Integrity Check
* Recovery
* Unsaved State handling
* Error Logs
* Safe Startup

### 10.5 Full QA

تست کامل:

```text
Functional
Database
Security
Offline
License
Backup
Restore
Printing
PDF
RTL
LTR
Persian
English
Pashto
Light Theme
Dark Theme
Installer
Update
Performance
```

### 10.6 Real-World Clinic Test

قبل از Release نهایی:

حداقل یک Workflow کامل:

```text
New Patient
→ Appointment
→ Examination
→ Dental Chart
→ Treatment Plan
→ Treatment
→ Invoice
→ Payment
→ Receipt
→ Backup
→ Close
→ Restore
```

از ابتدا تا انتها روی یک سیستم واقعی اجرا شود.

---

# Definition of Done — Artaveo Dental V1

در پایان این ۱۰ فاز، نرم‌افزار باید این ویژگی‌ها را داشته باشد:

```text
✅ Windows Desktop Application
✅ No Browser Dependency
✅ Offline Daily Operation
✅ Local SQLite
✅ Fast Indexed Database
✅ Multi-Clinic Architecture
✅ License System
✅ Trial System
✅ Installation Tracking
✅ Manual Revocation Support
✅ Automatic Online Verification
✅ Read-only Expiration State
✅ Daily Automated Backup
✅ USB Backup
✅ Cloud Backup Queue
✅ Backup Retention
✅ Restore
✅ A4 Compact Receipt
✅ Thermal Receipt Support
✅ Real-size PDF Receipt
✅ Full A4 Documents
✅ Persian Default
✅ English
✅ Pashto
✅ RTL/LTR
✅ Mixed-language Patient Data
✅ Light Theme
✅ Dark Theme
✅ Glassmorphism Design System
✅ Clinic Branding
✅ Patient Management
✅ Medical History
✅ Dental Chart
✅ Periodontal Chart
✅ Treatment Plans
✅ Clinical Notes
✅ Appointments
✅ Doctor Scheduling
✅ Staff & Permissions
✅ Billing
✅ Payments
✅ Debt/Installments
✅ Inventory
✅ Suppliers
✅ Dental Lab
✅ Reports
✅ Dashboard
✅ Notifications
✅ Audit Logs
✅ Automatic Updates
✅ Recovery
✅ Production Installer
```

### چیزهایی که عمداً در V1 داخل هسته محصول نمی‌ریزیم

مواردی مثل:

```text
Online Patient Portal
Online Booking
SMS Gateway
WhatsApp Integration
Multi-Branch Cloud Sync
Advanced Accounting
Insurance Integration
AI Features
Mobile App
```

از نظر معماری برای آینده قابل اضافه‌شدن هستند، اما نباید V1 را سنگین و پرهزینه کنند.

**اصل V1 این است: کامل برای عملیات داخل کلینیک، ولی نه شلوغ با سرویس‌های جانبی غیرضروری.**

---

# معماری داده — اصل مهم نهایی

برای اطلاعات مرجع:

```text
patient.gender_id = 1
patient.province_id = 10
patient.district_id = 27
patient.city_id = 84
```

نه:

```text
gender = "Male"
province = "Kabul"
district = "Bagrami"
```

و برای نمایش:

```text
reference ID
      ↓
translation table
      ↓
Current UI Language
```

بنابراین:

```text
ID 10
```

می‌تواند در رابط فارسی «کابل»، در انگلیسی «Kabul» و در پشتو «کابل» نمایش داده شود، بدون اینکه خود رکورد بیمار را برای هر زبان دوباره ذخیره کنیم.

برای داده‌های آزاد بیمار مثل نام، آدرس یا توضیحات، متن اصلی همان چیزی است که کاربر وارد کرده و به اجبار ترجمه یا Normalize نمی‌شود.

این ترکیب هم سرعت و Consistency را حفظ می‌کند، هم چندزبانه بودن واقعی را.

---

# اصل معماری که نباید در هیچ فاز نقض شود

**یک Codebase برای همه کلینیک‌ها.**

**Configuration به‌جای Fork.**

**Local Data برای عملیات.**

**Cloud فقط برای سرویس‌های مرکزی و قابلیت‌های اختیاری.**

**Domain نباید Dependency هسته برنامه باشد.**

**روزهای بدون اینترنت باید روز کاری عادی باشند.**

**هیچ Feature جدیدی نباید Security، Backup، License یا Data Integrity را دور بزند.**

**هیچ تغییر UI نباید باعث وابستگی Business Logic به Presentation Layer شود.**

**Database Schema باید Migration-Based و Versioned باشد.**

**Clinic Data، License Data و Control Data باید از هم جدا باشند.**

**تمام عملیات حساس Audit شوند.**

**هر چیزی که امروز برای یک کلینیک ساخته می‌شود، باید از نظر معماری قابلیت استفاده توسط کلینیک دوم را داشته باشد.**
