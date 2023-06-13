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
    lines.push("\t\tconst canvas = document.createElement('canvas');");
    lines.push("\t\tcanvas.id = 'roboscape-canvas';");
    lines.push("\t\telement.querySelector('content').appendChild(canvas);");
    lines.push("\t\tsetupDialog(element);");
    lines.push("\t\twindow.externalVariables['roboscapedialog'] = element;");
    lines.push("\t};");

    lines.push("\tscriptElement.setAttribute('src', 'https://gsteinltu.github.io/PseudoMorphic/script.js');");
    lines.push("\tdocument.head.appendChild(scriptElement);");



    // Restore end of document
    lines.push(end);

    // Overwrite existing
    content = lines.join("\n");
    let mut out_file = File::create("./index.js")?;
    out_file.write_all(content.as_bytes())?;
    
    Ok(())
}