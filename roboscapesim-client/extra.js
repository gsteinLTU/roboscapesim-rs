
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
            const observer = new ResizeObserver(function () {
                BABYLON.Engine.LastCreatedEngine.resize();
            });
            observer.observe(element);
            window.externalVariables['roboscapedialog'] = element;

            const buttonbar = document.createElement('div');
            buttonbar.id = 'roboscapebuttonbar';
            element.querySelector('content').appendChild(buttonbar);
        }

        // Create join dialog for later use
        {
            var element = createDialog('Join a Session');
            element.style.width = '400px';
            element.style.height = '200px';
            
            const content = document.createElement('div');
            content.id = 'roboscapejoincontent';
            let label = document.createElement('label');
            label.innerText = "Session ID: ";
            content.appendChild(label);
            const input = document.createElement('input');
            input.id = 'roboscapejoin';
            content.appendChild(input);
            element.querySelector('content').appendChild(content);

            const content2 = document.createElement('div');
            content.id = 'roboscapejoincontent2';
            label = document.createElement('label');
            label.innerText = "Recent Sessions: ";
            content2.appendChild(label);
            const dropdown = document.createElement('select');
            dropdown.id = 'roboscapejoin-dropdown';
            content2.appendChild(dropdown);
            element.querySelector('content').appendChild(content2);


            const content3 = document.createElement('div');
            content.id = 'roboscapejoincontent3';
            label = document.createElement('label');
            label.innerText = "Password: ";
            content3.appendChild(label);
            const password = document.createElement('input');
            dropdown.id = 'roboscapejoin-password';
            content3.appendChild(password);
            element.querySelector('content').appendChild(content3);

            setupDialog(element);
            window.externalVariables['roboscapedialog-join'] = element;
        }
    };

    scriptElement.setAttribute('src', 'https://gsteinltu.github.io/PseudoMorphic/script.js');
    document.head.appendChild(scriptElement);


    var scriptElement = document.createElement('script');
    scriptElement.async = false;