import os
import re
import glob

def fix_file(filepath):
    with open(filepath, 'r', encoding='utf-8') as f:
        content = f.read()

    conflict_pattern = re.compile(
        r'<<<<<<< HEAD\s*(.*?)\s*=======\s*(.*?)\s*>>>>>>> [^\n]+', 
        re.DOTALL
    )
    
    def replacer(match):
        head_content = match.group(1)
        branch_content = match.group(2)
        
        if 'revenue_splits' in head_content and 'resale_royalty_bps' in branch_content:
            head_fields = head_content.replace('};', '').replace('},', '').strip()
            branch_fields = branch_content.replace('};', '').replace('},', '').strip()
            
            combined = f"        {head_fields},\n        {branch_fields}\n    }};"
            
            after_bracket = branch_content.split('};')
            if len(after_bracket) > 1:
                combined += after_bracket[1]
                
            return combined
            
        elif 'capacity' in head_content and 'resale_royalty_bps' in branch_content:
            head_fields = head_content.replace('},', '').strip()
            return f"                {head_fields}\n            }},"
            
        else:
            return match.group(0)
            
    content = conflict_pattern.sub(replacer, content)
    
    invalid_clone_pattern = re.compile(
        r'(\.\.params\.clone\(\))\s*resale_royalty_bps:\s*0,\s*max_resale_price:\s*None,\s*allow_free_ticket_transfer:\s*false,',
        re.DOTALL
    )
    content = invalid_clone_pattern.sub(r'\1', content)
    
    invalid_capacity_pattern = re.compile(
        r'(capacity:\s*\d+,?(?:\s*//.*?)?)\s*resale_royalty_bps:\s*0,\s*max_resale_price:\s*None,\s*allow_free_ticket_transfer:\s*false,',
        re.DOTALL
    )
    content = invalid_capacity_pattern.sub(r'\1', content)

    with open(filepath, 'w', encoding='utf-8') as f:
        f.write(content)

base_dir = r"c:\projetcs\zicket-contract-zeus\contracts\event\src"
for f in glob.glob(os.path.join(base_dir, "test*.rs")):
    print(f"Fixing {f}")
    fix_file(f)
