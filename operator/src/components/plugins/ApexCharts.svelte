<script>
  import { onMount, onDestroy, afterUpdate } from 'svelte';
  import { appVariables } from '/src/stores/appVariables.js';

  let chart;
  let elm;
  let apex;
  
  function getApexConfiguration(appVariables) {
		return {
			title: {
				style: {
					fontSize: '14px',
					fontWeight: '600',
					fontFamily: appVariables.font.bodyFontFamily,
					color: appVariables.color.bodyColor
				}
			},
			legend: {
				fontFamily: appVariables.font.bodyFontFamily,
				labels: { colors: appVariables.color.bodyColor }
			},
			tooltip: {
				style: {
					fontSize: '12px',
					fontFamily: appVariables.font.bodyFontFamily
				}
			},
			grid: { borderColor: appVariables.color.borderColor },
			dataLabels: {
				style: {
					fontSize: '12px',
					fontFamily: appVariables.font.bodyFontFamily,
					fontWeight: '600',
					colors: undefined
				}
			},
			xaxis: {
				axisBorder: {
					show: true,
					color: appVariables.color.borderColor,
					height: 1,
					width: '100%',
					offsetX: 0,
					offsetY: -1
				},
				axisTicks: {
					show: true,
					borderType: 'solid',
					color: appVariables.color.borderColor,
					height: 6,
					offsetX: 0,
					offsetY: 0
				},
				labels: {
					style: {
						colors: appVariables.color.bodyColor,
						fontSize: '12px',
						fontFamily: appVariables.font.bodyFontFamily,
						fontWeight: appVariables.font.bodyFontWeight,
						cssClass: 'apexcharts-xaxis-label',
					}
				}
			},
			yaxis: {
				labels: {
					style: {
						colors: appVariables.color.bodyColor,
						fontSize: '12px',
						fontFamily: appVariables.font.bodyFontFamily,
						fontWeight: appVariables.font.bodyFontWeight,
						cssClass: 'apexcharts-xaxis-label',
					}
				}
			}
		};
	}
  
  function renderChart() {
  	if (apex && $$props.options && elm) {
			chart = new apex.default(elm, $$props.options);
			chart.render();
		}
  }

  onMount(async () => {
  	apex = await import ('apexcharts');
  	appVariables.subscribe(appVariables => {
  		if (appVariables.color) {
				Apex = getApexConfiguration(appVariables);
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
  });
  
  afterUpdate(() => {
		if (chart) {
			chart.destroy();
			renderChart();
		}
  });
</script>

<div bind:this={elm} style="height: {$$props.height}"></div>
