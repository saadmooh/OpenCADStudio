import subprocess
import json
import sys
import os

if sys.platform == "win32":
    try:
        sys.stdout.reconfigure(encoding='utf-8')
    except Exception:
        pass

PROJECT_DIR = r"c:\Users\j\Videos\OpenCADStudio-2026.41\OpenCADStudio-2026.41\projects\smart_village_masterplan"
DWG_PATH = os.path.join(PROJECT_DIR, "Al_Waha_Masterplan.dwg")
DXF_PATH = os.path.join(PROJECT_DIR, "Al_Waha_Masterplan.dxf")
XML_PATH = os.path.join(PROJECT_DIR, "Al_Waha_Masterplan.xml")
REPORT_PATH = os.path.join(PROJECT_DIR, "Al_Waha_Masterplan_Report.txt")

exe = r"C:\Program Files\Open CAD Studio\OpenCADStudio.exe"
print(f"Starting OpenCADStudio automation from: {exe}")

env = os.environ.copy()
env["OCS_HEADLESS"] = "1"

proc = subprocess.Popen(
    [exe, "--serve"],
    stdin=subprocess.PIPE,
    stdout=subprocess.PIPE,
    text=True,
    encoding="utf-8",
    bufsize=1,
    env=env
)

def send(req):
    line = json.dumps(req)
    proc.stdin.write(line + "\n")
    proc.stdin.flush()
    resp_line = proc.stdout.readline()
    if not resp_line:
        raise EOFError("OpenCADStudio process closed unexpectedly")
    return json.loads(resp_line.strip())

# Read startup banner
ready = json.loads(proc.stdout.readline())
print("OpenCADStudio Ready:", ready.get("name", "OCS"), ready.get("version", ""))

# Create clean new document
res_new = send({"op": "new"})
print("Initialized New CAD Document. Base entity count:", res_new.get("total", 0))

# Execute sequence of Civil 3D Parcel & Masterplan commands
commands = [
    # 1. Site initialization and scale
    ('PC_SITE_NEW "Al_Waha_Masterplan" 500 "Civil 3D Urban Master Plan based on gemini-svg"', "Initialize Site 'Al_Waha_Masterplan' at 1:500 Scale"),
    ('PC_SCALE 500', "Set Active Drawing Scale to 1:500"),

    # 2. Outer Development Green Boundary (265m x 190m)
    ('PC_CREATE_RECT Master_Boundary 265.0 190.0 0,0', "Create Master Site Perimeter Boundary (265.0m x 190.0m = 50,350 m²)"),
    ('PC_SET_STYLE Master_Boundary park', "Assign Park & Open Space Style to Master Boundary"),

    # 3. Government & Civic District (North-West)
    ('PC_CREATE_RECT Gov_Municipality 40.0 25.0 37.5,132.5', "Create Municipality Building (40m x 25m = 1,000 m²)"),
    ('PC_SET_STYLE Gov_Municipality civic', "Set Civic Zoning for Municipality"),
    ('PC_SET_PROP Gov_Municipality owner "Municipality of Al-Waha"', "Set Owner for Municipality"),
    ('PC_SET_PROP Gov_Municipality address "1st Avenue, Civic Center"', "Set Address for Municipality"),

    ('PC_CREATE_RECT Gov_Security 30.0 22.5 85.0,135.0', "Create Security & Emergency Center (30m x 22.5m = 675 m²)"),
    ('PC_SET_STYLE Gov_Security civic', "Set Civic Zoning for Security Center"),
    ('PC_SET_PROP Gov_Security owner "Ministry of Interior / Police"', "Set Owner for Security Center"),

    ('PC_CREATE_RECT Gov_Clinic 37.5 21.25 37.5,103.75', "Create Comprehensive Health Clinic (37.5m x 21.25m = 796.88 m²)"),
    ('PC_SET_STYLE Gov_Clinic civic', "Set Civic Zoning for Health Clinic"),
    ('PC_SET_PROP Gov_Clinic owner "Ministry of Health"', "Set Owner for Health Clinic"),

    ('PC_CREATE_RECT Gov_Parking 32.5 21.25 82.5,103.75', "Create Civic Parking Facility (32.5m x 21.25m = 690.63 m²)"),
    ('PC_SET_STYLE Gov_Parking civic', "Set Civic Zoning for Gov Parking"),

    # 4. Commercial & Retail District (North-East)
    ('PC_CREATE_RECT Shopping_Mall 57.5 32.5 150.0,125.0', "Create Commercial Shopping Mall (57.5m x 32.5m = 1,868.75 m²)"),
    ('PC_SET_STYLE Shopping_Mall commercial', "Set Commercial Zoning for Shopping Mall"),
    ('PC_SET_PROP Shopping_Mall owner "Al-Waha Commercial Real Estate"', "Set Owner for Shopping Mall"),

    ('PC_CREATE_RECT Local_Market 25.0 32.5 215.0,125.0', "Create Local Market & Retail Stalls (25m x 32.5m = 812.5 m²)"),
    ('PC_SET_STYLE Local_Market commercial', "Set Commercial Zoning for Local Market"),
    ('PC_SET_PROP Local_Market owner "Retail Traders Cooperative"', "Set Owner for Local Market"),

    ('PC_CREATE_RECT Mall_Parking 90.0 15.0 150.0,103.75', "Create Commercial Parking Plaza (90m x 15m = 1,350 m²)"),
    ('PC_SET_STYLE Mall_Parking commercial', "Set Commercial Zoning for Mall Parking"),

    # 5. Western Residential Quarter (South-West)
    ('PC_CREATE_RECT Villa_A1 27.5 22.5 37.5,57.5', "Create Residential Lot Villa A-1 (27.5m x 22.5m = 618.75 m²)"),
    ('PC_SET_STYLE Villa_A1 residential', "Set Residential Zoning for Villa A-1"),
    ('PC_CREATE_RECT Villa_A2 27.5 22.5 85.0,57.5', "Create Residential Lot Villa A-2 (27.5m x 22.5m = 618.75 m²)"),
    ('PC_SET_STYLE Villa_A2 residential', "Set Residential Zoning for Villa A-2"),
    ('PC_CREATE_RECT Villa_A3 27.5 22.5 37.5,27.5', "Create Residential Lot Villa A-3 (27.5m x 22.5m = 618.75 m²)"),
    ('PC_SET_STYLE Villa_A3 residential', "Set Residential Zoning for Villa A-3"),
    ('PC_CREATE_RECT Villa_A4 27.5 22.5 85.0,27.5', "Create Residential Lot Villa A-4 (27.5m x 22.5m = 618.75 m²)"),
    ('PC_SET_STYLE Villa_A4 residential', "Set Residential Zoning for Villa A-4"),

    # 6. Eastern Residential Quarter (South-East)
    ('PC_CREATE_RECT Villa_B1 27.5 22.5 155.0,57.5', "Create Residential Lot Villa B-1 (27.5m x 22.5m = 618.75 m²)"),
    ('PC_SET_STYLE Villa_B1 residential', "Set Residential Zoning for Villa B-1"),
    ('PC_CREATE_RECT Villa_B2 27.5 22.5 202.5,57.5', "Create Residential Lot Villa B-2 (27.5m x 22.5m = 618.75 m²)"),
    ('PC_SET_STYLE Villa_B2 residential', "Set Residential Zoning for Villa B-2"),
    ('PC_CREATE_RECT Villa_B3 27.5 22.5 155.0,27.5', "Create Residential Lot Villa B-3 (27.5m x 22.5m = 618.75 m²)"),
    ('PC_SET_STYLE Villa_B3 residential', "Set Residential Zoning for Villa B-3"),
    ('PC_CREATE_RECT Villa_B4 27.5 22.5 202.5,27.5', "Create Residential Lot Villa B-4 (27.5m x 22.5m = 618.75 m²)"),
    ('PC_SET_STYLE Villa_B4 residential', "Set Residential Zoning for Villa B-4"),

    # 7. Building Setbacks & Zoning Envelopes
    ('PC_SETBACK all 3.5', "Generate 3.5m Setback Envelopes on PC-SETBACK Layer"),

    # 8. Directional Spatial Renumbering (West-to-East)
    ('PC_RENUMBER "Al_Waha_Masterplan" "LOT-" 101 1 x', "Apply West-to-East Directional Renumbering"),

    # 9. Survey Mapcheck Precision Closure Analysis
    ('PC_MAPCHECK all', "Execute Survey Mapcheck Closure Verification on All Parcels"),

    # 10. Dynamic Centroid & Boundary Annotations
    ('PC_LABEL all 2.2', "Place Dynamic Lot Number & Area Centroid Labels (Scale 1:500)"),
    ('PC_LABEL_SEGS all tags 1.2', "Place Dynamic Segment Labels with Tag References (L1, L2...)"),

    # 11. Civil 3D Dynamic Tables (Cleanly stacked to the right of the site)
    ('PC_TABLE_AREA Al_Waha_Masterplan 280.0 130.0 1.8', "Generate Dynamic Parcel Area Table at (280, 130)"),
    ('PC_TABLE_LINES LOT-101 355.0 190.0 1.8', "Generate Dynamic Boundary Segment Line Table at (355, 190)"),
    ('PC_TABLE_CURVES LOT-101 355.0 120.0 1.8', "Generate Dynamic Curve Table at (355, 120)"),

    # 12. Master Plan Comprehensive Statistical Report
    ('PC_REPORT', "Generate Master Plan Statistical & Zoning Report"),

    # 13. LandXML 1.2 Export
    (f'PC_EXPORT_LANDXML "{XML_PATH}"', "Export Full Site Topology to Autodesk LandXML 1.2"),

    # 14. Thematic Zoning Hatching & CAD Legend
    ('PC_THEME zoning 0.35', "Generate Thematic Zoning Hatches & Graphic Legend")
]

step_num = 1
for cmd, desc in commands:
    print(f"[{step_num:02d}/{len(commands):02d}] {desc}...")
    res = send({"op": "run", "cmd": cmd})
    if not res.get("ok", False):
        print(f"  [ERROR] Command failed: {cmd} -> {res.get('error', 'unknown error')}")
    else:
        ent_count = res.get("entities", 0)
        print(f"  [OK] CAD entities: {ent_count}")
    step_num += 1

# Additional Civil Infrastructure Drafting (Roads, Roundabout, Parking Lines, North Arrow)
print("\nDrafting Infrastructure Network (Roundabout, Curbs, Stalls, North Arrow)...")
infra_commands = [
    # Roundabout Outer Ring (Radius 15m at 132.5, 95.0)
    'CIRCLE 132.5,95.0 15.0',
    # Roundabout Island Curb (Radius 9.5m)
    'CIRCLE 132.5,95.0 9.5',
    # Roundabout Lane Marking (Radius 12.5m)
    'CIRCLE 132.5,95.0 12.5',

    # Ring Road Outer Curbs
    'LINE 11.5,11.5 253.5,11.5',
    'LINE 253.5,11.5 253.5,178.5',
    'LINE 253.5,178.5 11.5,178.5',
    'LINE 11.5,178.5 11.5,11.5',

    'LINE 23.5,23.5 241.5,23.5',
    'LINE 241.5,23.5 241.5,166.5',
    'LINE 241.5,166.5 23.5,166.5',
    'LINE 23.5,166.5 23.5,23.5',

    # Central Vertical Boulevard Curbs
    'LINE 126.5,0.0 126.5,190.0',
    'LINE 138.5,0.0 138.5,190.0',
    # Central Horizontal Boulevard Curbs
    'LINE 0.0,89.0 265.0,89.0',
    'LINE 0.0,101.0 265.0,101.0',

    # Secondary Residential Streets
    'LINE 72.0,20.0 72.0,85.0',
    'LINE 78.0,20.0 78.0,85.0',
    'LINE 189.5,20.0 189.5,85.0',
    'LINE 195.5,20.0 195.5,85.0',

    # Commercial Parking Bays Lines (at Y=103.75 to 118.75)
    'LINE 165.0,103.75 165.0,118.75',
    'LINE 180.0,103.75 180.0,118.75',
    'LINE 195.0,103.75 195.0,118.75',
    'LINE 210.0,103.75 210.0,118.75',
    'LINE 225.0,103.75 225.0,118.75',

    # North Arrow Indicator at Top-Right
    'LINE 255.0,175.0 255.0,187.0',
    'LINE 255.0,187.0 252.0,180.0',
    'LINE 255.0,187.0 258.0,180.0',
    'TEXT 253.5,189.0 2.5 0 N'
]

for icmd in infra_commands:
    send({"op": "run", "cmd": icmd})

# Save DWG Drawing
print(f"\nSaving Production DWG to: {DWG_PATH}...")
res_dwg = send({"op": "save", "path": DWG_PATH})
print("DWG Save Result:", res_dwg)

# Save DXF Drawing
print(f"Saving Production DXF to: {DXF_PATH}...")
res_dxf = send({"op": "save", "path": DXF_PATH})
print("DXF Save Result:", res_dxf)

# Read Final Entities Count and Summary
summary = send({"op": "entities"})
print("\nFinal Drawing Entities Summary:")
print(json.dumps(summary, indent=2))

layers = send({"op": "layers"})
print("\nFinal Drawing Layers:")
print(json.dumps(layers, indent=2))

proc.stdin.close()
proc.wait()
print("\n=== MASTER PLAN PROJECT GENERATION COMPLETED SUCCESSFULLY ===")
