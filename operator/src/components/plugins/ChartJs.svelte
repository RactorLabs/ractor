<script>
	import Chart from 'chart.js/auto';
  import { onMount, onDestroy, afterUpdate } from 'svelte';
  import { appVariables } from '/src/stores/appVariables.js';

  let chart;
  let canvas;
  let unsubscribe;
  
  function renderChart() {
  	if (canvas) {
			chart = new Chart(canvas, {
				type: $$props.type,
				data: $$props.data,
				options: $$props.options
			});
  	}
  }

  onMount(() => {
  	unsubscribe = appVariables.subscribe(appVariables => {
  		if (appVariables.color) {
				Chart.defaults.font.family = appVariables.font.bodyFontFamily;
				Chart.defaults.font.size = 12;
				Chart.defaults.color = appVariables.color.bodyColor;
				Chart.defaults.borderColor = appVariables.color.borderColor;
				Chart.defaults.plugins.legend.display = false;
				Chart.defaults.plugins.tooltip.padding = { left: 8, right: 12, top: 8, bottom: 8 };
				Chart.defaults.plugins.tooltip.cornerRadius = 8;
				Chart.defaults.plugins.tooltip.titleMarginBottom = 6;
				Chart.defaults.plugins.tooltip.color = appVariables.color.componentBg;
				Chart.defaults.plugins.tooltip.multiKeyBackground = appVariables.color.componentColor;
				Chart.defaults.plugins.tooltip.backgroundColor = appVariables.color.componentColor;
				Chart.defaults.plugins.tooltip.titleFont.family = appVariables.font.bodyFontFamily;
				Chart.defaults.plugins.tooltip.titleFont.weight = appVariables.font.bodyFontWeight;
				Chart.defaults.plugins.tooltip.footerFont.family = appVariables.font.bodyFontFamily;
				Chart.defaults.plugins.tooltip.displayColors = true;
				Chart.defaults.plugins.tooltip.boxPadding = 6;
				Chart.defaults.scale.grid.color = appVariables.color.borderColor;
				Chart.defaults.scale.beginAtZero = true;
  		}
  		if (chart) {
  			chart.destroy();
  		}
    	renderChart();
		});
  });
  
  onDestroy(() => {
  	if (chart) {
  		chart.destroy();
  	}
  	if (unsubscribe) {
  		unsubscribe();
  	}
  });
  
  afterUpdate(() => {
		if (chart) {
			chart.destroy();
			renderChart();
		}
  });
</script>

<canvas bind:this={canvas} height={$$props.height}></canvas>
