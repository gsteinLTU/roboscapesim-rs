#[allow(unused_macros)]

use std::{error::Error, fs::File, io::{Read, Write}};

// Macro to allow build script to print output
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

    let end = lines.pop().unwrap();

    let modstart = lines.iter().position(|s| { s.contains("var s = document.createElement('script');")}).unwrap();
    let (lines, modlines) = lines.split_at(modstart);
    let mut lines = lines.iter().map(|s| { s.to_owned() }).collect::<Vec<_>>();
    let modlines = modlines.iter().map(|s| { "\t\t".to_owned() + s }).collect::<Vec<_>>();
    let mut modlines = modlines.iter().map(|x| &**x).collect();

    lines.push("");

    // Add CSS
    lines.push("\tvar element = document.createElement('link');");
    lines.push("\telement.setAttribute('rel', 'stylesheet');");
    lines.push("\telement.setAttribute('type', 'text/css');");
    lines.push("\telement.setAttribute('href', 'https://gsteinltu.github.io/PseudoMorphic/style.css');");
    lines.push("\tdocument.head.appendChild(element);");

    // Add JS
    lines.push("");
    lines.push("\tvar scriptElement = document.createElement('script');");

    // Create dialog for later use
    lines.push("");
    lines.push("\tscriptElement.onload = () => {");        
    lines.push("\t\tvar element = createDialog('RoboScape Online');");
    lines.push("\t\telement.style.width = '400px';");
    lines.push("\t\telement.style.height = '400px';");
    lines.push("\t\tconst canvas = document.createElement('canvas');");
    lines.push("\t\tcanvas.id = 'roboscape-canvas';");
    lines.push("\t\tcanvas.style.width = 'calc(100% - 32px)';");
    lines.push("\t\tcanvas.style.height = 'calc(100% - 64px)';");
    lines.push("\t\telement.querySelector('content').appendChild(canvas);");
    lines.push("\t\tsetupDialog(element);");
    lines.push("\t\tconst observer = new ResizeObserver(function () {");
    lines.push("\t\t    BABYLON.Engine.LastCreatedEngine.resize();");
    lines.push("\t\t});");
    lines.push("\t\tobserver.observe(element);");
    lines.push("\t\twindow.externalVariables['roboscapedialog'] = element;");
    lines.push("\t};");

    lines.push("\tscriptElement.setAttribute('src', 'https://gsteinltu.github.io/PseudoMorphic/script.js');");
    lines.push("\tdocument.head.appendChild(scriptElement);");

    lines.push("");
    lines.push("\tvar scriptElement = document.createElement('script');");
    lines.push("\tscriptElement.async = false;");
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

    lines.push("\t};");    

    lines.push("\tscriptElement.setAttribute('src', 'https://preview.babylonjs.com/babylon.js');");
    lines.push("\tdocument.head.appendChild(scriptElement);");
    lines.push("\tdisableRetinaSupport();");


    // Restore end of document
    lines.push(end);

    // Overwrite existing
    content = lines.join("\n");
    let mut out_file = File::create("./index.js")?;
    out_file.write_all(content.as_bytes())?;
    
    Ok(())
}