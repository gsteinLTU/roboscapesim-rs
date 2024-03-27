
use std::{error::Error, fs::File, io::{Read, Write}};

// Macro to allow build script to print output
#[allow(unused_macros)]
macro_rules! warn {
    ($($tokens: tt)*) => {
        println!("cargo:warning={}", format!($($tokens)*))
    }
}

fn main() -> Result<(), Box<dyn Error>> {    
    // basic extension setup
    let r = netsblox_extension_util::build();
    if r.is_err() {
        return r
    }

    // add additional file includes
    // Read file  
    let mut content = String::new();
    {
        let mut file = File::open("./index.js")?;
        file.read_to_string(&mut content)?;
    }

    let mut lines = content.split("\n").collect::<Vec<_>>();

    // Hold onto end for now
    let end = lines.pop().unwrap();

    // Find js code running module
    let modstart = lines.iter().position(|s| { s.contains("var s = document.createElement('script');")}).unwrap();
    let (lines, modlines) = lines.split_at(modstart);
    let mut lines = lines.iter().map(|s| { s.to_owned() }).collect::<Vec<_>>();
    let modlines = modlines.iter().map(|s| { "\t\t".to_owned() + s }).collect::<Vec<_>>();
    let mut modlines = modlines.iter().map(|x| &**x).collect();

    lines.push("");

    // Read file  
    let mut extra_content = String::new();
    {
        let mut file = File::open("./extra.js")?;
        file.read_to_string(&mut extra_content)?;
        
        for line in extra_content.split("\n") {
            lines.push(line);
        }
        
    }
    
    lines.push("");
    
    lines.push("\tscriptElement.onload = () => {");    
    lines.push("\t\tvar loaderScriptElement = document.createElement('script');");
    lines.push("\t\tloaderScriptElement.async = false;");
    
    lines.push("\t\tloaderScriptElement.onload = () => {");

    // Put module loading here to not init until Babylon loads
    lines.append(&mut modlines);

    lines.push("\t\t};");

    lines.push("\t\tloaderScriptElement.setAttribute('src', 'https://preview.babylonjs.com/loaders/babylonjs.loaders.min.js');");
    lines.push("\t\tdocument.head.appendChild(loaderScriptElement);");

    lines.push("\t\tvar guiScriptElement = document.createElement('script');");
    lines.push("\t\tguiScriptElement.async = false;");
    lines.push("\t\tguiScriptElement.setAttribute('src', 'https://preview.babylonjs.com/gui/babylon.gui.js');");
    lines.push("\t\tdocument.head.appendChild(guiScriptElement);");

    lines.push("\t};");    
    
    lines.push("\tscriptElement.setAttribute('src', 'https://preview.babylonjs.com/babylon.js');");
    lines.push("\tdocument.head.appendChild(scriptElement);");
    //lines.push("\tdisableRetinaSupport();");


    // Restore end of document
    lines.push(end);

    // Overwrite existing
    content = lines.join("\n");
    let mut out_file = File::create("./index.js")?;
    out_file.write_all(content.as_bytes())?;
    
    Ok(())
}