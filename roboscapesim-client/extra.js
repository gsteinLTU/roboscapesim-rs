
    // Add CSS
    var element = document.createElement('link');
    element.setAttribute('rel', 'stylesheet');
    element.setAttribute('type', 'text/css');
    element.setAttribute('href', 'https://gsteinltu.github.io/PseudoMorphic/style.css');
    document.head.appendChild(element);

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
            canvas.style.height = 'calc(100% - 32px)';
            element.querySelector('content').appendChild(canvas);
            setupDialog(element);
            
            window.externalVariables['roboscapedialog'] = element;

            const buttonbar = document.createElement('div');
            buttonbar.id = 'roboscapebuttonbar';
            element.querySelector('content').appendChild(buttonbar);
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

            element = createDialog('Create a Session', false, ['Create', 'Close']);
            element.querySelector('content').innerHTML += `
            <div style="margin-bottom: 12px;"><label>Password:&nbsp;</label><input class="inset"/></div>
            <div><label>Environment:&nbsp;</label><input class="inset"/></div>
            `;

            setupDialog(element, false);
            window.externalVariables['roboscapedialog-new'] = element;
        }
    };

    scriptElement.setAttribute('src', 'https://gsteinltu.github.io/PseudoMorphic/script.js');
    document.head.appendChild(scriptElement);
 
    var scriptElement = document.createElement('script');
    scriptElement.async = false;