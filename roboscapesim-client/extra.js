
    // Add CSS
    var element = document.createElement('link');
    element.setAttribute('rel', 'stylesheet');
    element.setAttribute('type', 'text/css');
    element.setAttribute('href', 'https://gsteinltu.github.io/PseudoMorphic/style.css');
    document.head.appendChild(element);

    // Add JS
    var scriptElement = document.createElement('script');

    // Create dialog for later use
    scriptElement.onload = () => {        
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

    };

    scriptElement.setAttribute('src', 'https://gsteinltu.github.io/PseudoMorphic/script.js');
    document.head.appendChild(scriptElement);


    var scriptElement = document.createElement('script');
    scriptElement.async = false;