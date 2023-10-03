
    // Add CSS
    var element = document.createElement('link');
    element.setAttribute('rel', 'stylesheet');
    element.setAttribute('type', 'text/css');
    element.setAttribute('href', 'https://gsteinltu.github.io/PseudoMorphic/style.css');
    document.head.appendChild(element);

    var extraStyle = document.createElement('style');
    extraStyle.innerText = `
    #roboscapebuttonbar * {
        margin: auto 5px;
    }
    `;
    document.head.appendChild(extraStyle);

    // Add JS
    var scriptElement = document.createElement('script');

    scriptElement.onload = () => {        
        // Create 3D view dialog for later use
        {
            var element = createDialog('RoboScape Online');
            element.style.width = '400px';
            element.style.height = '400px';
            const canvas = document.createElement('canvas');
            canvas.id = 'roboscape-canvas';
            canvas.style.width = 'calc(100% - 32px)';
            canvas.style.height = 'calc(100% - 48px)';
            element.querySelector('content').appendChild(canvas);
            setupDialog(element);
            
            window.externalVariables['roboscapedialog'] = element;

            
            const buttonbar = document.createElement('div');
            buttonbar.id = 'roboscapebuttonbar';
            element.querySelector('content').appendChild(buttonbar);
            
            const robotmenu_label = document.createElement('label');
            robotmenu_label.innerText = 'Robot ID:';
            buttonbar.appendChild(robotmenu_label);
            const robotmenu = document.createElement('select');
            robotmenu.className = 'inset';
            robotmenu.onmousedown = (e) => { e.stopPropagation(); };
            const nonchoice = document.createElement('option');
            robotmenu.appendChild(nonchoice);
            buttonbar.appendChild(robotmenu);
            window.externalVariables['roboscapedialog-robotmenu'] = robotmenu;
        }

        // Create join dialog for later use
        {
            var element = createDialog('Join a Session', false, ['Join', 'Close']);
            element.querySelector('content').innerHTML += `
            <div style="margin-bottom: 12px;"><label>Room ID:&nbsp;</label><input class="inset"/></div>
            <div><label>Password:&nbsp;</label><input class="inset"/></div>
            `;

            setupDialog(element, false);
            window.externalVariables['roboscapedialog-join'] = element;

            element = createDialog('Create a Session', false, ['Create', 'Edit Mode', 'Close']);
            element.querySelector('content').innerHTML += `
            <div style="margin-bottom: 12px;"><label>Environment:&nbsp;</label><input id="roboscapedialog-new-environment" class="inset"/></div>
            <div><label>Password:&nbsp;</label><input id="roboscapedialog-new-password" class="inset"/></div>
            `;

            setupDialog(element, false);
            window.externalVariables['roboscapedialog-new'] = element;
        }
    };

    scriptElement.setAttribute('src', 'https://gsteinltu.github.io/PseudoMorphic/script.js');
    document.head.appendChild(scriptElement);
 
    var scriptElement = document.createElement('script');
    scriptElement.async = false;